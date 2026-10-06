use clap::Parser;
#[derive(Parser)]
struct Args {
    #[arg(long,default_value="127.0.0.1")] host: String,
    #[arg(long,default_value_t=8080)] port: u16,
    #[arg(long,default_value_t=8,value_parser=clap::value_parser!(u8).range(1..=64))] max_players: u8,
    #[arg(long,default_value_t=1800)] room_ttl: u64,
    #[arg(long,default_value_t=180)] race_timeout: u64,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    let args=Args::parse();
    let listener=tokio::net::TcpListener::bind((args.host.as_str(),args.port)).await?;
    tracing::info!(address=%listener.local_addr()?,"server listening");
    code_racer_server::serve(listener,args.max_players as usize,args.room_ttl,args.race_timeout).await
}
