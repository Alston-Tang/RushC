mod common;

use crate::common::{
    get_mongodb_client, AirflowArchiveLock, AirflowVideoLock, BiliArchiveInfo, BiliVideoInfo,
};
use anyhow::Error;
use biliup::uploader::bilibili::{BiliBili, ResponseData, Studio, Vid, Video};
use biliup::uploader::credential::login_by_cookies;
use common::check_lock_acquired;
use log::{error, info};
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::Collection;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use structopt::StructOpt;

pub struct VideoInfo {
    pub obj_id: String,
    pub lock_id: String,
    pub path: String,
}

impl FromStr for VideoInfo {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let pair = value.split(":").collect::<Vec<&str>>();
        if pair.len() != 3 {
            panic!("unknown video info string: {value}");
        }
        Ok(VideoInfo {
            obj_id: pair[0].to_string(),
            lock_id: pair[1].to_string(),
            path: pair[2].to_string(),
        })
    }
}

impl fmt::Debug for VideoInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VideoInfo")
            .field("obj_id", &self.obj_id)
            .field("lock_id", &self.lock_id)
            .field("path", &self.path)
            .finish()
    }
}

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(long)]
    vid: Option<String>,
    #[structopt(parse(from_os_str), long)]
    cookie: PathBuf,
    #[structopt(long)]
    mongodb_uri: String,
    #[structopt(long, default_value = "airflow")]
    db: String,
    #[structopt(long, default_value = "airflow_video_lock")]
    video_lock_collection: String,
    #[structopt(long, default_value = "airflow_archive_lock")]
    archive_lock_collection: String,
    #[structopt(long)]
    archive_obj_id: String,
    #[structopt(long)]
    archive_lock: String,
    #[structopt(long, default_value = "bili_archive_info")]
    archive_output_collection: String,
    #[structopt(long, default_value = "bili_video_info")]
    video_output_collection: String,
    #[structopt(long)]
    title: Option<String>,
    #[structopt(long, default_value = "虚拟UP主,动画,综合,直播录像,七海Nana7mi,七海,虚拟主播,VUP")]
    tag: String,
    #[structopt(long)]
    cover: Option<PathBuf>,
    #[structopt(use_delimiter = true)]
    videos: Vec<VideoInfo>,
}

async fn check_all_locks_required(
    archive_obj_id: &str,
    archive_lock: &str,
    videos_info: &Vec<VideoInfo>,
    archive_lock_collection: &Collection<AirflowArchiveLock>,
    video_lock_collection: &Collection<AirflowVideoLock>,
) -> bool {
    if !check_lock_acquired(archive_lock_collection, &archive_lock, &archive_obj_id).await {
        error!(
            "lock of archive {} has not been acquired by {}",
            archive_lock, archive_obj_id
        );
        return false;
    }
    for video in videos_info {
        if !check_lock_acquired(video_lock_collection, &video.lock_id, &video.obj_id).await {
            error!(
                "lock of video {}({}) has not been acquired by {}",
                video.obj_id, video.path, video.lock_id
            );
            return false;
        }
    }
    true
}

async fn build_archive_studio(
    bili: &BiliBili,
    vid: &Option<String>,
    title: &Option<String>,
    tag: &str,
) -> Studio {
    let studio = match vid {
        Some(vid_str) => {
            let mut exist_studio = bili.studio_data(&Vid::Bvid(vid_str.clone())).await.unwrap();
            if title.is_some() {
                exist_studio.title = title.clone().unwrap();
            }
            exist_studio
        }
        None => Studio {
            copyright: 2,
            source: "https://live.bilibili.com/21452505".to_string(),
            tid: 47,
            cover: "".to_string(),
            title: title.clone().unwrap_or("".to_string()),
            desc_format_id: 0,
            desc: "七海Nana7mi：https://space.bilibili.com/434334701".to_string(),
            desc_v2: None,
            dynamic: "".to_string(),
            subtitle: Default::default(),
            tag: tag.to_string(),
            videos: vec![],
            dtime: None,
            open_subtitle: false,
            interactive: 0,
            mission_id: None,
            dolby: 0,
            lossless_music: 0,
            no_reprint: 0,
            open_elec: 0,
            aid: None,
            up_selection_reply: false,
            up_close_reply: false,
            up_close_danmu: false,
        },
    };
    studio
}

pub async fn cover_up(studio: &mut Studio, bili: &BiliBili, cover: PathBuf) -> () {
    let url = bili.cover_up(&std::fs::read(cover).unwrap()).await.unwrap();
    info!("cover is uploaded to {url}");
    studio.cover = url;
}

pub async fn construct_videos_list(videos: &Vec<VideoInfo>) -> Vec<Video> {
    videos
        .iter()
        .map(|video| Video {
            title: None,
            filename: video.path.clone(),
            desc: "".to_string(),
        })
        .collect()
}

struct SubmitResponse {
    aid: i64,
    bvid: String,
}

fn parse_submit_response(res: ResponseData) -> SubmitResponse {
    let data = res.data.unwrap();
    let aid = data
        .as_object()
        .unwrap()
        .get("aid")
        .unwrap()
        .as_i64()
        .unwrap();
    let bvid = data
        .as_object()
        .unwrap()
        .get("bvid")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    SubmitResponse { aid, bvid }
}

fn parse_edit_response(res: serde_json::Value) -> SubmitResponse {
    let data = res.as_object().unwrap().get("data").unwrap();
    let aid = data
        .as_object()
        .unwrap()
        .get("aid")
        .unwrap()
        .as_i64()
        .unwrap();
    let bvid = data
        .as_object()
        .unwrap()
        .get("bvid")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    SubmitResponse { aid, bvid }
}

async fn submit(
    videos: &Vec<VideoInfo>,
    cookie: &Path,
    vid: &Option<String>,
    title: Option<String>,
    tag: &str,
    cover: Option<PathBuf>,
) -> SubmitResponse {
    info!("get user credential from cookie file");
    let bili = login_by_cookies(cookie).await.unwrap();
    info!(
        "user: {}",
        bili.my_info().await.unwrap()["data"]["name"]
            .as_str()
            .unwrap()
    );
    let mut studio = build_archive_studio(&bili, &vid, &title, &tag).await;
    if cover.is_some() {
        cover_up(&mut studio, &bili, cover.unwrap()).await;
    }
    studio.videos = construct_videos_list(videos).await;
    info!("studio: {:?}", studio);
    if vid.is_some() {
        info!("editing existing archive {}", vid.as_ref().unwrap());
        let res = bili.edit(&studio).await.unwrap();
        info!("{:?}", res);
        parse_edit_response(res)
    } else {
        info!("adding a new archive");
        let res = bili.submit(&studio).await.unwrap();
        info!("{:?}", res);
        parse_submit_response(res)
    }
}

async fn update_archive_video_info(
    edit: bool,
    aid: i64,
    bvid: String,
    archive_obj_id: String,
    videos: Vec<VideoInfo>,
    archive_info_collection: Collection<BiliArchiveInfo>,
    video_info_collection: Collection<BiliVideoInfo>,
) -> () {
    info!("update bili_archive_info collection");
    info!(
        "aid={}, bvid={}, archive_obj_id={}",
        aid, bvid, archive_obj_id
    );
    if !edit {
        let archive_obj_id = ObjectId::from_str(archive_obj_id.as_str()).unwrap();
        let archive_obj = BiliArchiveInfo {
            archive_id: archive_obj_id,
            aid: aid.clone(),
            bvid: bvid.clone(),
        };
        archive_info_collection
            .insert_one(archive_obj, None)
            .await
            .unwrap();
    }
    for video in videos {
        let video_obj_id = ObjectId::from_str(video.obj_id.as_str()).unwrap();
        let filter = doc! {"DocId": video_obj_id};
        let update = doc! {"$set": {"Bvid": bvid.clone()}};
        video_info_collection
            .update_one(filter, update, None)
            .await
            .unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    ftlog::builder().try_init().unwrap();
    let opts = Opts::from_args();
    let client = get_mongodb_client(&opts.mongodb_uri).await;
    let video_lock_collection = client
        .database(&opts.db)
        .collection::<AirflowVideoLock>(&opts.video_lock_collection);
    let archive_lock_collection = client
        .database(&opts.db)
        .collection::<AirflowArchiveLock>(&opts.archive_lock_collection);
    info!("checking if all necessary locks are acquired");
    let lock_acquired = check_all_locks_required(
        &opts.archive_obj_id,
        &opts.archive_lock,
        &opts.videos,
        &archive_lock_collection,
        &video_lock_collection,
    )
    .await;
    if !lock_acquired {
        panic!("some locks are not already acquired");
    }
    let res = submit(&opts.videos, &opts.cookie, &opts.vid, opts.title, &opts.tag, opts.cover).await;
    let video_info_collection = client
        .database(&opts.db)
        .collection::<BiliVideoInfo>(&opts.video_output_collection);
    let archive_info_collection = client
        .database(&opts.db)
        .collection::<BiliArchiveInfo>(&opts.archive_output_collection);
    update_archive_video_info(
        opts.vid.is_some(),
        res.aid,
        res.bvid,
        opts.archive_obj_id,
        opts.videos,
        archive_info_collection,
        video_info_collection,
    )
    .await;
    Ok(())
}
