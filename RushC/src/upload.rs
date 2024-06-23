use anyhow::Error;
use biliup::client::StatelessClient;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use structopt::clap::arg_enum;
use structopt::StructOpt;
use tracing::info;

use biliup::uploader::credential::login_by_cookies;
use biliup::uploader::{bilibili, line, VideoFile};
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

// this structure should be consistent to structure defined in biliup-rs
// biliup-rs/crates/bin/cli.rs
arg_enum! {
    #[derive(Debug)]
    enum Line {
        Qn,
        Bda2,
    }
}

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(parse(from_os_str))]
    file: PathBuf,
    #[structopt(long, possible_values=&Line::variants(), case_insensitive=false, default_value="Qn")]
    line: Line,
    #[structopt(long, default_value = "10")]
    limit: u32,
    #[structopt(parse(from_os_str), long)]
    cookie: PathBuf,
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
                info!("{} bytes out of {} bytes have been uploaded", uploaded_size, file_size);
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
    let file_path = opts.file;
    let cookie_path = opts.cookie;
    let remote_video = upload_video(&file_path, &cookie_path, opts.line, opts.limit as usize)
        .await
        .unwrap();
    info!(
        "file {} has been uploaded to remote as {}",
        file_path.display(),
        remote_video.filename
    );
    log::logger().flush();
    Result::Ok(())
}
