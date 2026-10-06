use code_racer_protocol::*;
use futures_util::{SinkExt,StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
pub struct Network {pub tx:mpsc::Sender<ClientMessage>,pub rx:mpsc::Receiver<Result<ServerMessage,String>>, task:tokio::task::JoinHandle<()>}
impl Drop for Network {fn drop(&mut self) {self.task.abort();}}
impl Network {
    pub async fn connect(url:&str)->anyhow::Result<Self> {
        let (socket,_)=tokio::time::timeout(std::time::Duration::from_secs(5),tokio_tungstenite::connect_async(url)).await??;
        let (mut sink,mut stream)=socket.split();
        let (tx,mut outgoing)=mpsc::channel::<ClientMessage>(32);let (incoming,rx)=mpsc::channel(64);
        let task=tokio::spawn(async move {
            loop {tokio::select! {
                msg=outgoing.recv()=>{let Some(msg)=msg else {break};let Ok(json)=serde_json::to_string(&msg) else {continue};if sink.send(Message::Text(json.into())).await.is_err() {break;}}
                msg=stream.next()=>{match msg {Some(Ok(Message::Text(json)))=>{match serde_json::from_str(&json) {Ok(msg)=>{if incoming.send(Ok(msg)).await.is_err() {return;}},Err(_)=>{let _=incoming.send(Err("Invalid server response".into())).await;}}},Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,_=>{}}}
            }}
            let _=incoming.send(Err("Connection lost".into())).await;
        });
        Ok(Self {tx,rx,task})
    }
    pub fn send(&self,msg:ClientMessage)->Result<(),String> {self.tx.try_send(msg).map_err(|_|"Connection busy or closed".into())}
}
