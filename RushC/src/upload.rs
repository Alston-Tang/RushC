mod common;

use anyhow::{Context, Error};
use biliup::client::StatelessClient;
use common::{AirflowVideoLock, BiliVideoInfo, PollStream};
use futures::StreamExt;
use log::info;
use mongodb::bson::doc;
use mongodb::{Client, Collection};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use structopt::clap::arg_enum;
use structopt::StructOpt;

use biliup::uploader::credential::login_by_cookies;
use biliup::uploader::{bilibili, line, VideoFile};
use mongodb::bson::oid::ObjectId;

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
    lock_id: String,
    #[structopt(long)]
    file_obj_id: String,
    #[structopt(long)]
    mongodb_uri: String,
    #[structopt(long, default_value = "airflow")]
    db: String,
    #[structopt(long, default_value = "airflow_video_lock")]
    lock_collection: String,
    #[structopt(long, default_value = "bili_video_info")]
    output_collection: String,
}

async fn get_mongodb_client(mongodb_uri: &str) -> Client {
    info!("connecting to mongodb {mongodb_uri}");
    Client::with_uri_str(mongodb_uri).await.unwrap()
}

async fn check_lock_acquired(
    collection: &Collection<AirflowVideoLock>,
    lock_id: &str,
    file_obj_id: &str,
) -> bool {
    info!("checking if file {file_obj_id} has been locked by {lock_id}");
    collection
        .find_one(doc! {"LockId": ObjectId::from_str(lock_id).unwrap(), "DocId": ObjectId::from_str(file_obj_id).unwrap()}, None)
        .await
        .unwrap()
        .is_some()
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
    let mut uploaded_bytes_count = Arc::new(AtomicUsize::new(0));
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
                info!(limit=3000; "{}", counter.load(Relaxed));
            }
        )
        .await
        .unwrap();
    Some(remote_video)
}

async fn update_bili_video_info(
    file_obj_id: &str,
    video: &bilibili::Video,
    collection: Collection<BiliVideoInfo>,
) -> () {
    info!("updating bili_video_info collection");
    info!(
        "title={}, filename={}, desc={}",
        video.title.clone().unwrap_or(String::new()),
        video.filename,
        video.desc
    );
    let file_obj_id = ObjectId::from_str(file_obj_id).unwrap();
    let video_obj = BiliVideoInfo {
        doc_id: file_obj_id,
        title: video.title.clone(),
        filename: video.filename.clone(),
        desc: video.desc.clone(),
    };
    collection.insert_one(video_obj, None).await.unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    ftlog::builder().try_init().unwrap();
    let opts = Opts::from_args();
    let client = get_mongodb_client(&opts.mongodb_uri).await;
    let file_path = opts.file;
    let lock_id = opts.lock_id;
    let file_obj_id = opts.file_obj_id;
    let cookie_path = opts.cookie;
    let video_lock_collection = client
        .database(&opts.db)
        .collection::<AirflowVideoLock>(&opts.lock_collection);
    if !check_lock_acquired(&video_lock_collection, &lock_id, &file_obj_id).await {
        panic!("lock for video {}({file_obj_id}) expected to already acquire lock {lock_id} but actually not.", file_path.display());
    }
    let remote_video = upload_video(&file_path, &cookie_path, opts.line, opts.limit as usize)
        .await
        .unwrap();
    update_bili_video_info(
        &file_obj_id,
        &remote_video,
        client
            .database(&opts.db)
            .collection::<BiliVideoInfo>(&opts.output_collection),
    )
    .await;
    info!(
        "file {} has been uploaded to remote as {}",
        file_path.display(),
        remote_video.filename
    );
    log::logger().flush();
    Result::Ok(())
}
