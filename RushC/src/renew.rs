use std::path::PathBuf;
use anyhow::Error;
use structopt::StructOpt;
use tracing::info;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use biliup::uploader::credential::login_by_cookies;

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(parse(from_os_str), long)]
    config: PathBuf,
}

async fn renew(config: PathBuf) -> Result<(), Error> {
    info!("Attempting to renew credentials...");
    let bili = login_by_cookies(&config, None).await?;
    let info = bili.my_info().await?;
    info!("Cookie for user {} refreshed", info["data"]["name"]);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry().with(fmt::layer()).init();
    let opts = Opts::from_args();

    renew(opts.config).await?;
    
    Ok(())
} 