use std::{fs,path::PathBuf};
use anyhow::Context;
use directories::ProjectDirs;
use serde::{Deserialize,Serialize};
use crate::typing::Stats;
#[derive(Clone,Serialize,Deserialize)]
#[serde(default)]
pub struct Config {
    pub username:String,pub language:String,pub theme:String,pub default_mode:String,pub word_count:usize,pub multiplayer:Multiplayer,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(default)]
pub struct Multiplayer {pub server:String}
impl Default for Multiplayer {fn default()->Self {Self {server:"ws://127.0.0.1:8080".into()}}}
impl Default for Config {fn default()->Self {Self {username:String::new(),language:"english".into(),theme:"editor".into(),default_mode:"words".into(),word_count:50,multiplayer:Multiplayer::default()}}}
#[derive(Clone,Serialize,Deserialize)]
pub struct Record {pub date:String,pub mode:String,pub language:String,pub stats:Stats}
pub struct Storage {pub config:Config,pub history:Vec<Record>,config_path:PathBuf,history_path:PathBuf}
impl Storage {
    pub fn load()->anyhow::Result<Self> {
        let dirs=ProjectDirs::from("","","code-racer").context("Cannot locate user directories")?;
        let config_path=dirs.config_dir().join("config.toml");let history_path=dirs.data_local_dir().join("history.json");
        let config=if config_path.exists() {toml::from_str(&fs::read_to_string(&config_path)?).context("Invalid config.toml")?} else {Config::default()};
        let history=if history_path.exists() {serde_json::from_str(&fs::read_to_string(&history_path)?).context("Invalid history.json (file preserved)")?} else {Vec::new()};
        Ok(Self {config,history,config_path,history_path})
    }
    pub fn save_config(&self)->anyhow::Result<()> {write(&self.config_path,toml::to_string_pretty(&self.config)?.as_bytes())}
    pub fn record(&mut self,mode:&str,language:&str,stats:Stats)->anyhow::Result<()> {
        self.history.push(Record {date:chrono::Utc::now().to_rfc3339(),mode:mode.into(),language:language.into(),stats});
        write(&self.history_path,&serde_json::to_vec_pretty(&self.history)?)
    }
}
fn write(path:&std::path::Path,bytes:&[u8])->anyhow::Result<()> {
    if let Some(parent)=path.parent() {fs::create_dir_all(parent)?;}
    let tmp=path.with_extension("tmp");fs::write(&tmp,bytes)?;
    // Windows cannot rename over an existing file. Keep a recoverable backup.
    let backup=path.with_extension("bak");
    if path.exists() {if backup.exists() {fs::remove_file(&backup)?;} fs::rename(path,&backup)?;}
    if let Err(e)=fs::rename(&tmp,path) {if backup.exists() {let _=fs::rename(&backup,path);}return Err(e.into());}
    Ok(())
}
