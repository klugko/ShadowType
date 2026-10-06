use std::time::{Duration,Instant};
use code_racer_protocol::*;
use crossterm::event::{KeyCode,KeyEvent,KeyModifiers};
use rand::seq::IndexedRandom;
use crate::{Args,Command,network::Network,storage::Storage,typing::Session};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum State {Home,SoloConfig,SoloRace,MultiplayerMenu,CreateRoom,JoinRoom,Lobby,Countdown,MultiplayerRace,Results,Settings,Help,History,Username}
pub struct App {
    pub state:State,pub storage:Storage,pub session:Option<Session>,pub network:Option<Network>,pub room:Option<Snapshot>,pub id:String,
    pub selected:usize,pub mode:String,pub language:String,pub words:usize,pub seconds:u64,pub input:String,pub error:String,pub quit:bool,pub dirty:bool,
    pub offset:i64,pending:Option<Command>,saved:bool,last_progress:Instant,last_display:Instant,
}
impl App {
    pub fn new(args:Args)->anyhow::Result<Self> {
        let mut storage=Storage::load()?;if let Some(server)=args.server {storage.config.multiplayer.server=server;}
        let mut app=Self {state:State::Home,mode:storage.config.default_mode.clone(),language:storage.config.language.clone(),words:storage.config.word_count.clamp(1,1000),seconds:30,storage,session:None,network:None,room:None,id:String::new(),selected:0,input:String::new(),error:String::new(),quit:false,dirty:true,offset:0,pending:args.command,saved:false,last_progress:Instant::now(),last_display:Instant::now()};
        if !valid_username(&app.storage.config.username) {app.state=State::Username;} else {app.apply_pending();}
        Ok(app)
    }
    fn apply_pending(&mut self) {
        match self.pending.take() {
            Some(Command::Solo {mode,language,words,seconds})=>{self.mode=mode;self.language=language;self.words=words as usize;self.seconds=seconds as u64;self.start_solo();}
            Some(Command::Multiplayer)=>self.state=State::MultiplayerMenu,
            Some(Command::Create)=>self.state=State::CreateRoom,
            Some(Command::Join {code})=>{self.input=code.to_uppercase();self.state=State::JoinRoom;}
            None=>self.state=State::Home,
        }
    }
    pub fn start_solo(&mut self) {
        let source=text(&self.language).unwrap_or_else(||text("english").unwrap_or("Practice makes progress."));
        let target=match self.mode.as_str() {
            "words"|"time"=>{let bank:Vec<_>=source.split_whitespace().collect();let n=if self.mode=="time" {2000} else {self.words};let mut rng=rand::rng();(0..n).filter_map(|_|bank.choose(&mut rng).copied()).collect::<Vec<_>>().join(" ")}
            _=>source.into(),
        };
        self.session=Some(Session::new(&target,if self.mode=="time" {Some(Duration::from_secs(self.seconds))} else {None}));self.state=State::SoloRace;self.saved=false;self.error.clear();
    }
    fn send(&mut self,msg:ClientMessage) {if let Some(net)=&self.network {if let Err(e)=net.send(msg) {self.error=e;}}}
    async fn connect(&mut self,create:bool) {
        if !create && !valid_code(&self.input) {self.error="Invalid room code".into();return;}
        match Network::connect(&self.storage.config.multiplayer.server).await {
            Ok(net)=>{self.network=Some(net);self.room=None;self.error.clear();let username=self.storage.config.username.clone();self.send(if create {ClientMessage::Create {version:VERSION,username,language:self.language.clone()}} else {ClientMessage::Join {version:VERSION,username,code:self.input.clone()}});}
            Err(e)=>self.error=format!("Server unavailable: {e}"),
        }
    }
    fn leave(&mut self) {self.send(ClientMessage::Leave);self.network=None;self.room=None;self.session=None;self.state=State::Home;self.selected=0;}
    pub async fn key(&mut self,key:KeyEvent) {
        self.error.clear();
        if matches!(self.state,State::SoloRace|State::MultiplayerRace) {
            match key.code {
                KeyCode::Esc=>{if self.state==State::MultiplayerRace {self.leave();} else {self.state=State::Home;self.session=None;}}
                KeyCode::Backspace=>{if let Some(s)=&mut self.session {s.backspace();}}
                KeyCode::Enter=>{if let Some(s)=&mut self.session {s.input('\n');}}
                KeyCode::Tab=>{if let Some(s)=&mut self.session {for _ in 0..4 {s.input(' ');}}}
                KeyCode::Char(ch) if !key.modifiers.intersects(KeyModifiers::CONTROL|KeyModifiers::ALT)=>{if let Some(s)=&mut self.session {s.input(ch);}}
                _=>{}
            }return;
        }
        if matches!(self.state,State::Username|State::JoinRoom) || (self.state==State::Settings && self.selected==0) {
            match key.code {
                KeyCode::Esc=>{if self.state==State::Username {self.quit=true;} else {self.state=State::Home;self.selected=0;}}
                KeyCode::Enter=>{
                    if self.state==State::JoinRoom {self.connect(false).await;}
                    else if valid_username(&self.input) {self.storage.config.username=self.input.clone();if let Err(e)=self.storage.save_config() {self.error=e.to_string();} else if self.state==State::Username {self.apply_pending();} else {self.selected=1;}}
                    else {self.error="Username must contain 1–24 printable characters".into();}
                }
                KeyCode::Backspace=>{self.input.pop();}
                KeyCode::Char(ch)=>{if self.input.chars().count()<24 {self.input.push(if self.state==State::JoinRoom {ch.to_ascii_uppercase()} else {ch});}}
                _=>{}
            }return;
        }
        if key.code==KeyCode::Esc {if self.room.is_some() {self.leave();} else {self.state=State::Home;self.selected=0;}return;}
        if key.code==KeyCode::Char('q') {self.quit=true;return;}
        if key.code==KeyCode::Char('?') {self.state=State::Help;return;}
        match self.state {
            State::Home=>match key.code {
                KeyCode::Char('j')|KeyCode::Down=>self.selected=(self.selected+1)%6,
                KeyCode::Char('k')|KeyCode::Up=>self.selected=(self.selected+5)%6,
                KeyCode::Char('s')=>{self.state=State::SoloConfig;self.selected=0;}
                KeyCode::Char('m')=>{self.state=State::MultiplayerMenu;self.selected=0;}
                KeyCode::Enter=>{self.state=match self.selected {0=>State::SoloConfig,1=>State::MultiplayerMenu,2=>State::History,3=>State::Settings,4=>State::Help,_=>{self.quit=true;State::Home}};self.selected=0;if self.state==State::Settings {self.input=self.storage.config.username.clone();}}
                _=>{}
            },
            State::SoloConfig=>match key.code {
                KeyCode::Char('j')|KeyCode::Down=>self.selected=(self.selected+1)%5,
                KeyCode::Char('k')|KeyCode::Up=>self.selected=(self.selected+4)%5,
                KeyCode::Char('h')|KeyCode::Left=>self.configure(false),
                KeyCode::Char('l')|KeyCode::Right=>self.configure(true),
                KeyCode::Enter=>self.start_solo(),_=>{}
            },
            State::MultiplayerMenu=>match key.code {
                KeyCode::Char('c')=>{self.state=State::CreateRoom;self.connect(true).await;}
                KeyCode::Char('j')=>{self.state=State::JoinRoom;self.input.clear();}
                KeyCode::Up|KeyCode::Down=>self.selected=1-self.selected.min(1),
                KeyCode::Enter=>{if self.selected==0 {self.state=State::CreateRoom;self.connect(true).await;} else {self.state=State::JoinRoom;self.input.clear();}},_=>{}
            },
            State::CreateRoom=>{if key.code==KeyCode::Enter {self.connect(true).await;}},
            State::Lobby=>match key.code {
                KeyCode::Char('r')|KeyCode::Enter=>{let ready=self.room.as_ref().and_then(|r|r.players.iter().find(|p|p.id==self.id)).is_some_and(|p|p.ready);self.send(ClientMessage::Ready {ready:!ready});}
                KeyCode::Char('s')=>self.send(ClientMessage::Start),_=>{}
            },
            State::Results=>match key.code {
                KeyCode::Char('r')|KeyCode::Char('l')=>{if self.room.is_some() {self.send(ClientMessage::Again);} else {self.start_solo();}},_=>{}
            },
            State::Settings=>match key.code {
                KeyCode::Char('j')|KeyCode::Down=>self.selected=(self.selected+1)%3,
                KeyCode::Char('k')|KeyCode::Up=>self.selected=(self.selected+2)%3,
                KeyCode::Char('l')|KeyCode::Right|KeyCode::Enter=>{if self.selected==1 {self.storage.config.theme=cycle(&self.storage.config.theme,&["editor","mono","dark"],true);} else {self.language=cycle(&self.language,&["english","french"],true);self.storage.config.language=self.language.clone();}if let Err(e)=self.storage.save_config() {self.error=e.to_string();}},_=>{}
            },
            State::History=>match key.code {KeyCode::Char('j')|KeyCode::Down=>self.selected=self.selected.saturating_add(1).min(self.storage.history.len().saturating_sub(1)),KeyCode::Char('k')|KeyCode::Up=>self.selected=self.selected.saturating_sub(1),_=>{}},
            _=>{}
        }
    }
    fn configure(&mut self,forward:bool) {
        match self.selected {
            0=>{self.mode=cycle(&self.mode,&["words","time","quote","code"],forward);if self.mode=="code" {self.language="rust".into();} else if !["english","french"].contains(&self.language.as_str()) {self.language="english".into();}}
            1=>self.language=cycle(&self.language,if self.mode=="code" {&["rust","python","typescript","sql"]} else {&["english","french"]},forward),
            2=>{let counts=[10,25,50,100];let i=counts.iter().position(|&n|n==self.words).unwrap_or(2);self.words=counts[(i+if forward {1} else {3})%4];}
            3=>{let values=[15,30,60,120];let i=values.iter().position(|&n|n==self.seconds).unwrap_or(1);self.seconds=values[(i+if forward {1} else {3})%4];}
            _=>{}
        }
    }
    pub fn message(&mut self,msg:Result<ServerMessage,String>) {
        match msg {
            Ok(ServerMessage::Welcome {version,player_id})=>{if version!=VERSION {self.error="Unsupported server version".into();self.network=None;} else {self.id=player_id;self.send(ClientMessage::Ping {client_ms:now_ms()});}}
            Ok(ServerMessage::Error {message})=>self.error=message,
            Ok(ServerMessage::Pong {client_ms,server_ms})=>self.offset=server_ms as i64-((now_ms()+client_ms)/2) as i64,
            Ok(ServerMessage::Room(room))=>{
                if self.room.as_ref().is_none_or(|r|r.start_ms!=room.start_ms) && room.start_ms.is_some() {self.session=Some(Session::new(&room.text,None));self.saved=false;}
                self.language=room.language.clone();self.state=match room.phase {Phase::Waiting=>State::Lobby,Phase::Countdown=>State::Countdown,Phase::Racing=>State::MultiplayerRace,Phase::Finished=>State::Results};
                if room.phase==Phase::Racing {if let (Some(s),Some(start))=(&mut self.session,room.start_ms) {if s.started.is_none() {let elapsed=(now_ms() as i64+self.offset-start as i64).max(0) as u64;s.started=Instant::now().checked_sub(Duration::from_millis(elapsed));}}}
                if room.phase==Phase::Finished {if let Some(s)=&mut self.session {s.finished.get_or_insert_with(Instant::now);}self.save_result();}self.room=Some(room);
            }
            Err(e)=>{self.error=e;self.network=None;if matches!(self.state,State::Countdown|State::MultiplayerRace|State::Lobby) {self.state=State::MultiplayerMenu;self.room=None;}}
        }
    }
    fn save_result(&mut self) {
        if self.saved {return;}if let Some(s)=&self.session {let mode=if self.room.is_some() {"multiplayer".into()} else {format!("{}/{}",self.mode,if self.mode=="time" {self.seconds as usize} else {self.words})};if let Err(e)=self.storage.record(&mode,&self.language,s.stats()) {self.error=e.to_string();}self.saved=true;}
    }
    pub fn tick(&mut self) {
        if matches!(self.state,State::SoloRace|State::MultiplayerRace) {
            if let Some(s)=&mut self.session {s.tick();}
            if self.last_progress.elapsed()>=Duration::from_millis(100) && self.state==State::MultiplayerRace {if let Some(s)=&self.session {let st=s.stats();self.send(ClientMessage::Progress {position:st.position,errors:st.errors,correct:st.correct,attempts:st.attempts});}self.last_progress=Instant::now();}
            if self.session.as_ref().is_some_and(Session::done) {self.save_result();if self.state==State::SoloRace {self.state=State::Results;}}
            if self.last_display.elapsed()>=Duration::from_millis(100) {self.dirty=true;self.last_display=Instant::now();}
        }
        if self.state==State::Countdown {self.dirty=true;}
    }
}
fn cycle(current:&str,values:&[&str],forward:bool)->String {let i=values.iter().position(|&v|v==current).unwrap_or(0);values[(i+if forward {1} else {values.len()-1})%values.len()].into()}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn app()->App {
        App {state:State::Home,storage:Storage::for_test(),session:None,network:None,room:None,id:String::new(),selected:0,mode:"words".into(),language:"english".into(),words:10,seconds:15,input:String::new(),error:String::new(),quit:false,dirty:true,offset:0,pending:None,saved:false,last_progress:Instant::now(),last_display:Instant::now()}
    }
    #[tokio::test]
    async fn navigation_and_solo_transitions() {
        let mut a=app();a.key(KeyCode::Char('s').into()).await;assert_eq!(a.state,State::SoloConfig);
        a.key(KeyCode::Enter.into()).await;assert_eq!(a.state,State::SoloRace);
        let target=a.session.as_ref().expect("session").target.concat();assert_eq!(target.split_whitespace().count(),10);
        a.key(KeyCode::Char('q').into()).await;assert!(!a.quit);assert_eq!(a.session.as_ref().expect("session").typed,"q");
        a.key(KeyCode::Esc.into()).await;assert_eq!(a.state,State::Home);
        a.mode="code".into();a.language="rust".into();a.start_solo();assert!(a.session.as_ref().expect("code").target.concat().contains("\n    "));
    }
    #[test]
    fn disconnect_is_recoverable() {
        let mut a=app();a.state=State::MultiplayerRace;a.message(Err("Connection lost".into()));assert_eq!(a.state,State::MultiplayerMenu);assert!(a.network.is_none());assert_eq!(a.error,"Connection lost");
    }
}
