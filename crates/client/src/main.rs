mod app;
mod network;
mod storage;
mod typing;
mod ui;
use clap::{Parser,Subcommand};
use crossterm::{event::{Event,EventStream,KeyCode,KeyModifiers,KeyEventKind},execute,terminal::{enable_raw_mode,disable_raw_mode,EnterAlternateScreen,LeaveAlternateScreen}};
use futures_util::StreamExt;
use ratatui::{Terminal,backend::CrosstermBackend};
use std::io;
#[derive(Parser)]
#[command(version,about="An editor-shaped terminal typing trainer")]
pub struct Args {
    #[arg(long,global=true)] server:Option<String>,
    #[command(subcommand)] command:Option<Command>,
}
#[derive(Subcommand)]
pub enum Command {
    Solo {#[arg(long,default_value="words",value_parser=["words","time","quote","code"])] mode:String,#[arg(long,default_value="english",value_parser=["english","french","rust","python","javascript","typescript","sql"])] language:String,#[arg(long,default_value_t=50,value_parser=clap::value_parser!(u16).range(1..=1000))] words:u16,#[arg(long,default_value_t=30,value_parser=clap::value_parser!(u16).range(1..=3600))] seconds:u16},
    Multiplayer,Create,Join {code:String},
}
struct Guard;
impl Drop for Guard {fn drop(&mut self) {let _=disable_raw_mode();let _=execute!(io::stdout(),LeaveAlternateScreen,crossterm::cursor::Show);}}
#[tokio::main]
async fn main()->anyhow::Result<()> {
    let args=Args::parse();let mut app=app::App::new(args)?;
    let old=std::panic::take_hook();std::panic::set_hook(Box::new(move |info| {let _=disable_raw_mode();let _=execute!(io::stdout(),LeaveAlternateScreen,crossterm::cursor::Show);old(info);}));
    enable_raw_mode()?;let _guard=Guard;execute!(io::stdout(),EnterAlternateScreen)?;
    let mut terminal=Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut events=EventStream::new();let mut tick=tokio::time::interval(std::time::Duration::from_millis(50));
    while !app.quit {
        if app.dirty {terminal.draw(|frame|ui::draw(frame,&app))?;app.dirty=false;}
        tokio::select! {
            event=events.next()=>{match event {Some(Ok(Event::Key(key))) if key.kind!=KeyEventKind::Release=>{if key.code==KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {app.quit=true;} else {app.key(key).await;} app.dirty=true;},Some(Ok(Event::Resize(..)))=>app.dirty=true,Some(Err(e))=>return Err(e.into()),None=>break,_=>{}}}
            message=async {if let Some(net)=&mut app.network {net.rx.recv().await} else {std::future::pending().await}}=>{if let Some(msg)=message {app.message(msg);} else {app.network=None;app.error="Connection lost".into();}app.dirty=true;}
            _=tick.tick()=>app.tick(),
        }
    }
    Ok(())
}
