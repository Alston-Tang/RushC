mod common;

use anyhow::Error;
use biliup::client::StatelessClient;
use common::{BiliVideoInfo, PollStream};
use futures::StreamExt;
use log::info;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use structopt::clap::arg_enum;
use structopt::StructOpt;

use biliup::uploader::credential::login_by_cookies;
use biliup::uploader::{bilibili, line, VideoFile};

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
    #[structopt(long)]
    execution_summary_path: Option<PathBuf>,
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
    info!("pre-uploading video file {}", file.display());
    let uploader = line.pre_upload(&bili, file_obj).await.unwrap();
    let client = StatelessClient::default();
    info!("start uploading video file {}", file.display());
    let remote_video = uploader
        .upload(
            client,
            limit,
            |vs| {
                vs.map(|chunk| {
                    let chunk = chunk?;
                    let len = chunk.len();
                    Ok((PollStream::new(chunk), len))
                })
            },
            |counter: Arc<AtomicUsize>| {
                info!(limit=3000; "{} bytes uploaded", counter.load(Relaxed));
            },
        )
        .await
        .unwrap();
    Some(remote_video)
}

fn gen_execution_summary(video: &bilibili::Video, out_path: PathBuf) -> () {
    info!("generating execution summary");
    info!(
        "title={}, filename={}, desc={}",
        video.title.clone().unwrap_or(String::new()),
        video.filename,
        video.desc
    );
    let video_obj = BiliVideoInfo {
        title: video.title.clone(),
        filename: video.filename.clone(),
        desc: video.desc.clone(),
    };
    let out_file = File::create(out_path).unwrap();
    let mut writer = BufWriter::new(out_file);
    serde_json::to_writer(&mut writer, &video_obj).unwrap();
    writer.flush().unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    ftlog::builder().try_init().unwrap();
    let opts = Opts::from_args();
    let file_path = opts.file;
    let cookie_path = opts.cookie;
    let remote_video = upload_video(&file_path, &cookie_path, opts.line, opts.limit as usize)
        .await
        .unwrap();
    if opts.execution_summary_path.is_some() {
        gen_execution_summary(&remote_video, opts.execution_summary_path.unwrap());
    }
    info!(
        "file {} has been uploaded to remote as {}",
        file_path.display(),
        remote_video.filename
    );
    log::logger().flush();
    Result::Ok(())
}
