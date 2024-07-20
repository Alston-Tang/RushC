use anyhow::Error;
use biliup::client::StatelessClient;
use futures::StreamExt;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tracing::info;

use biliup::uploader::credential::login_by_cookies;
use biliup::uploader::{bilibili, line, VideoFile};
use serde::{Deserialize, Serialize};
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

// this structure should be consistent to structure defined in biliup-rs
// biliup-rs/crates/bin/cli.rs

#[derive(Serialize, Deserialize)]
enum Line {
    Qn,
    Bda2,
    Ws, 
    Bldsa,
    Tx, 
    Txa, 
    Bda
}

#[derive(Serialize, Deserialize)]
struct UploadConfig {
    file: PathBuf,
    line: Line,
    limit: u32,
    cookie: PathBuf,
}

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(parse(from_os_str), long)]
    config: PathBuf,
    #[structopt(parse(from_os_str), long)]
    out: Option<PathBuf>,
}

async fn upload_video(
    file: &Path,
    cookie: &Path,
    line: Line,
    limit: usize,
) -> Option<bilibili::Video> {
    info!("get user credential from cookie file");
    let bili = login_by_cookies(cookie).await.unwrap();
    info!(
        "user: {}",
        bili.my_info().await.unwrap()["data"]["name"]
            .as_str()
            .unwrap()
    );
    let line = match line {
        Line::Qn => line::qn(),
        Line::Bda2 => line::bda2(),
        Line::Ws => line::ws(),
        Line::Bldsa => line::bldsa(),
        Line::Tx => line::tx(),
        Line::Txa => line::txa(),
        Line::Bda => line::bda()
    };
    info!("using upload line {:?}", line);
    info!("opening video file {}", file.display());
    let file_obj: VideoFile = VideoFile::new(file).unwrap();

    let file_size = file_obj.file.metadata().unwrap().len();
    let mut uploaded_size: i64 = 0;

    info!("pre-uploading video file {}", file.display());
    let uploader = line.pre_upload(&bili, file_obj).await.unwrap();
    let client = StatelessClient::default();
    info!("start uploading video file {}", file.display());
    let remote_video = uploader
        .upload(client, limit, |vs| {
            vs.map(|chunk| {
                info!(
                    "{} bytes out of {} bytes have been uploaded",
                    uploaded_size, file_size
                );
                let chunk = chunk?;
                let len = chunk.len();
                uploaded_size += len as i64;
                Ok((chunk, len))
            })
        })
        .await
        .unwrap();
    Some(remote_video)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry().with(fmt::layer()).init();
    let opts = Opts::from_args();

    info!("using config {}", opts.config.display());
    let config_file = File::open(opts.config)?;
    let reader = BufReader::new(config_file);

    let config: UploadConfig = serde_json::from_reader(reader)?;
    info!("{}", serde_json::to_string(&config).unwrap());


    let remote_video = upload_video(
        &config.file,
        &config.cookie,
        config.line,
        config.limit as usize,
    )
    .await
    .unwrap();
    info!(
        "file {} has been uploaded to remote as {}",
        config.file.display(),
        remote_video.filename
    );

    match opts.out {
        Some(path) => {
            let file = OpenOptions::new().write(true).create(true).open(path)?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &remote_video)?;
        },
        None => {},
    }

    Result::Ok(())
}
