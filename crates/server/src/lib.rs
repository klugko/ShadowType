use std::{collections::HashMap, sync::Arc, time::{Duration,Instant}};
use code_racer_protocol::*;
use futures_util::{SinkExt,StreamExt};
use tokio::{net::TcpListener,sync::{Mutex,mpsc}};
use tokio_tungstenite::tungstenite::{Message,protocol::WebSocketConfig};
use uuid::Uuid;

type Sender=mpsc::Sender<ServerMessage>;
struct Room { view: Snapshot, members: HashMap<String,Sender>, touched: Instant, start: Option<Instant> }
struct Hub { rooms: HashMap<String,Room>, membership: HashMap<String,String>, max: usize, ttl: Duration, timeout: Duration }
impl Room {
    fn broadcast(&mut self) {
        self.view.server_ms=now_ms();
        let message=ServerMessage::Room(self.view.clone());
        for sender in self.members.values() { let _=sender.try_send(message.clone()); }
    }
    fn reset(&mut self) {
        self.view.phase=Phase::Waiting; self.view.start_ms=None; self.start=None;
        self.view.players.retain(|p| p.connected);
        for p in &mut self.view.players {p.ready=false;p.position=0;p.errors=0;p.wpm=0.0;p.accuracy=100.0;p.finished_ms=None;}
    }
    fn finish_if_done(&mut self) {
        if self.view.phase==Phase::Racing && self.view.players.iter().all(|p| !p.connected || p.finished_ms.is_some()) { self.view.phase=Phase::Finished; }
    }
}
impl Hub {
    fn leave(&mut self,id:&str) {
        let Some(code)=self.membership.remove(id) else {return};
        let Some(room)=self.rooms.get_mut(&code) else {return};
        room.members.remove(id);
        if matches!(room.view.phase,Phase::Waiting|Phase::Finished) {room.view.players.retain(|p|p.id!=id);} else if let Some(p)=room.view.players.iter_mut().find(|p|p.id==id) {p.connected=false;}
        if room.view.host==id {room.view.host=room.view.players.iter().find(|p|p.connected).map(|p|p.id.clone()).unwrap_or_default();}
        room.touched=Instant::now(); room.finish_if_done(); room.broadcast();
        if room.members.is_empty() {self.rooms.remove(&code);}
    }
    fn handle(&mut self,id:&str,tx:&Sender,msg:ClientMessage) -> Result<(),String> {
        match msg {
            ClientMessage::Ping {client_ms} => {let _=tx.try_send(ServerMessage::Pong {client_ms,server_ms:now_ms()}); return Ok(());}
            ClientMessage::Leave => {self.leave(id);return Ok(());}
            ClientMessage::Create {version,username,language} => {
                if version!=VERSION || !valid_username(&username) {return Err("Invalid version or username".into());}
                let target=text(&language).ok_or("Unsupported language")?;
                if self.rooms.len()>=1024 {return Err("Server room limit reached".into());}
                self.leave(id);
                let mut code=room_code(); while self.rooms.contains_key(&code) {code=room_code();}
                let view=Snapshot {code:code.clone(),host:id.into(),players:vec![player(id,username)],phase:Phase::Waiting,text:target.into(),language,start_ms:None,server_ms:now_ms()};
                self.rooms.insert(code.clone(),Room {view,members:HashMap::from([(id.into(),tx.clone())]),touched:Instant::now(),start:None});
                self.membership.insert(id.into(),code.clone());
                if let Some(room)=self.rooms.get_mut(&code) {room.broadcast();} return Ok(());
            }
            ClientMessage::Join {version,code,username} => {
                if version!=VERSION || !valid_username(&username) || !valid_code(&code) {return Err("Invalid version, room code or username".into());}
                let room=self.rooms.get(&code).ok_or("Room not found")?;
                if room.view.phase!=Phase::Waiting {return Err("Race already started".into());}
                if room.view.players.len()>=self.max {return Err("Room is full".into());}
                self.leave(id);
                let room=self.rooms.get_mut(&code).ok_or("Room no longer exists")?;
                room.view.players.push(player(id,username));room.members.insert(id.into(),tx.clone());room.touched=Instant::now();
                self.membership.insert(id.into(),code); room.broadcast(); return Ok(());
            }
            _=>{}
        }
        let code=self.membership.get(id).ok_or("Join a room first")?;
        let room=self.rooms.get_mut(code).ok_or("Room not found")?;
        match msg {
            ClientMessage::Ready {ready} => {
                if room.view.phase!=Phase::Waiting {return Err("Room is not waiting".into());}
                if let Some(p)=room.view.players.iter_mut().find(|p|p.id==id) {p.ready=ready;}
            }
            ClientMessage::Start => {
                if room.view.host!=id {return Err("Only the host can start".into());}
                if room.view.phase!=Phase::Waiting || !room.view.players.iter().all(|p|p.ready) {return Err("All players must be ready".into());}
                room.view.phase=Phase::Countdown;room.view.start_ms=Some(now_ms()+3000);room.start=Some(Instant::now()+Duration::from_secs(3));
            }
            ClientMessage::Again => {
                if room.view.host!=id || room.view.phase!=Phase::Finished {return Err("Only the host can reset a finished race".into());}
                room.reset();
            }
            ClientMessage::Progress {position,errors,correct,attempts} => {
                if room.view.phase!=Phase::Racing {return Err("Race is not running".into());}
                let length=grapheme_count(&room.view.text);
                if position>length || correct>position || attempts<position || attempts>1_000_000 || errors>attempts || correct>attempts.saturating_sub(errors) {return Err("Invalid progress".into());}
                let elapsed=room.start.map(|s|s.elapsed()).unwrap_or_default();
                let p=room.view.players.iter_mut().find(|p|p.id==id).ok_or("Player not found")?;
                if p.finished_ms.is_some() {return Ok(());}
                if errors<p.errors {return Err("Error count cannot decrease".into());}
                p.position=position;p.errors=errors;
                p.wpm=if elapsed.as_secs_f64()>0.0 {correct as f64*12.0/elapsed.as_secs_f64()} else {0.0};
                p.accuracy=if attempts==0 {100.0} else {(attempts-errors) as f64/attempts as f64*100.0};
                if position==length {p.finished_ms=Some(elapsed.as_millis() as u64);}
                room.finish_if_done();
            }
            _=>return Err("Unexpected message".into()),
        }
        room.touched=Instant::now(); room.broadcast(); Ok(())
    }
    fn tick(&mut self) {
        let mut expired=Vec::new();
        for (code,room) in &mut self.rooms {
            if room.touched.elapsed()>self.ttl {expired.push(code.clone());continue;}
            if room.view.phase==Phase::Countdown && room.start.is_some_and(|s|Instant::now()>=s) {room.view.phase=Phase::Racing;room.broadcast();}
            if room.view.phase==Phase::Racing && room.start.is_some_and(|s|s.elapsed()>=self.timeout) {room.view.phase=Phase::Finished;room.broadcast();}
        }
        for code in expired {if let Some(room)=self.rooms.remove(&code) {for id in room.members.keys() {self.membership.remove(id);} for tx in room.members.values() {let _=tx.try_send(ServerMessage::Error {message:"Room expired".into()});}}}
    }
}
fn player(id:&str,name:String)->Player {Player {id:id.into(),name,ready:false,connected:true,position:0,errors:0,wpm:0.0,accuracy:100.0,finished_ms:None}}

pub async fn serve(listener:TcpListener,max:usize,ttl:u64,timeout:u64)->anyhow::Result<()> {
    let hub=Arc::new(Mutex::new(Hub {rooms:HashMap::new(),membership:HashMap::new(),max,ttl:Duration::from_secs(ttl),timeout:Duration::from_secs(timeout)}));
    let clock=hub.clone();
    let timer=tokio::spawn(async move {let mut tick=tokio::time::interval(Duration::from_millis(50));loop {tick.tick().await;clock.lock().await.tick();}});
    loop {
        tokio::select! {
            result=listener.accept()=>{
                let (socket,_)=match result {Ok(v)=>v,Err(e)=>{timer.abort();return Err(e.into());}};
                let hub=hub.clone();
                tokio::spawn(async move {
                    let config=WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
                    let handshake=tokio::time::timeout(Duration::from_secs(5),tokio_tungstenite::accept_async_with_config(socket,Some(config))).await;
                    let Ok(Ok(ws))=handshake else {return};
                    let id=Uuid::new_v4().to_string();let (tx,mut rx)=mpsc::channel(64);
                    let (mut sink,mut stream)=ws.split();
                    let _=tx.try_send(ServerMessage::Welcome {version:VERSION,player_id:id.clone()});
                    let writer=tokio::spawn(async move {while let Some(msg)=rx.recv().await {let Ok(json)=serde_json::to_string(&msg) else {continue}; if sink.send(Message::Text(json.into())).await.is_err() {break;}}});
                    let mut budget=0;let mut window=Instant::now();
                    while let Some(Ok(msg))=stream.next().await {
                        if window.elapsed()>=Duration::from_secs(1) {budget=0;window=Instant::now();} budget+=1;if budget>100 {break;}
                        match msg {
                            Message::Text(json)=>{
                                let result=match serde_json::from_str::<ClientMessage>(&json) {Ok(msg)=>hub.lock().await.handle(&id,&tx,msg),Err(_)=>Err("Invalid protocol message".into())};
                                if let Err(message)=result {let _=tx.try_send(ServerMessage::Error {message});}
                            }
                            Message::Close(_)=>break,
                            _=>{}
                        }
                    }
                    hub.lock().await.leave(&id);writer.abort();
                });
            }
            _=tokio::signal::ctrl_c()=>{timer.abort();return Ok(());}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hub()->Hub {Hub {rooms:HashMap::new(),membership:HashMap::new(),max:2,ttl:Duration::from_secs(30),timeout:Duration::from_secs(10)}}
    #[test]
    fn room_limits_validation_and_cleanup() {
        let mut h=hub();let (tx,_rx)=mpsc::channel(64);
        assert!(h.handle("a",&tx,ClientMessage::Create {version:VERSION+1,username:"A".into(),language:"english".into()}).is_err());
        h.handle("a",&tx,ClientMessage::Create {version:VERSION,username:"A".into(),language:"english".into()}).expect("create");
        let code=h.membership["a"].clone();
        h.handle("b",&tx,ClientMessage::Join {version:VERSION,code:code.clone(),username:"B".into()}).expect("join");
        assert!(h.handle("c",&tx,ClientMessage::Join {version:VERSION,code:code.clone(),username:"C".into()}).is_err());
        assert!(h.handle("b",&tx,ClientMessage::Again).is_err());
        h.rooms.get_mut(&code).expect("room").touched=Instant::now()-Duration::from_secs(31);h.tick();
        assert!(h.rooms.is_empty());assert!(h.membership.is_empty());
    }
    #[test]
    fn last_player_disconnect_removes_room() {
        let mut h=hub();let (tx,_rx)=mpsc::channel(64);
        h.handle("a",&tx,ClientMessage::Create {version:VERSION,username:"A".into(),language:"english".into()}).expect("create");
        h.leave("a");assert!(h.rooms.is_empty());assert!(h.membership.is_empty());
    }
}
