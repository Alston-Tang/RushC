use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use anyhow::Error;
use biliup::bilibili::Vid;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use structopt::StructOpt;
use tracing::info;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use biliup::uploader::credential::login_by_cookies;


#[derive(Serialize, Deserialize)]
struct ShowConfig {
    vid: String,
    cookie: PathBuf,
}

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(parse(from_os_str), long)]
    config: PathBuf,
    #[structopt(parse(from_os_str), long)]
    out: Option<PathBuf>,
}


async fn show(config: &ShowConfig) -> Result<Value, Error> {
    let bilibili = login_by_cookies(&config.cookie, None).await?;
    let vid = Vid::Bvid(config.vid.clone());
    let video_info = bilibili.video_data(&vid, None).await?;

    Ok(video_info)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry().with(fmt::layer()).init();
    let opts = Opts::from_args();

    info!("using config {}", opts.config.display());
    let config_file = File::open(opts.config)?;
    let reader = BufReader::new(config_file);

    let config: ShowConfig = serde_json::from_reader(reader)?;

    let archive_info = show(&config).await.unwrap();

    info!("{}", serde_json::to_string_pretty(&archive_info)?);

    match opts.out {
        Some(path) => {
            let file = OpenOptions::new().write(true).create(true).open(path)?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &archive_info)?;
        },
        None => {},
    }
    Result::Ok(())
}