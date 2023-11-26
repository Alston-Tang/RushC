mod common;

use crate::common::{
    AirflowArchiveLock, AirflowVideoLock, BiliArchiveInfo, BiliVideoInfo,
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
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

pub struct VideoInfo {
    pub path: String,
}

impl FromStr for VideoInfo {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(VideoInfo {
            path: value.to_string(),
        })
    }
}

impl fmt::Debug for VideoInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VideoInfo")
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
    #[structopt(long, default_value = "airflow")]
    db: String,
    #[structopt(long)]
    title: Option<String>,
    #[structopt(long, default_value = "虚拟UP主,动画,综合,直播录像,七海Nana7mi,七海,虚拟主播,VUP")]
    tag: String,
    #[structopt(long)]
    cover: Option<PathBuf>,
    #[structopt(use_delimiter = true)]
    videos: Vec<VideoInfo>,
    #[structopt(long)]
    execution_summary_path: Option<PathBuf>,
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

#[derive(Serialize, Deserialize)]
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



fn gen_execution_summary(result: SubmitResponse, out_path: PathBuf) -> () {
    info!("generating execution summary");
    info!(
        "aid={}, bvid={}",
        result.aid,
        result.bvid
    );
    let summary = serde_json::to_string(&result);
    let out_file = File::create(out_path).unwrap();
    let mut writer = BufWriter::new(out_file);
    writer.flush().unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    ftlog::builder().try_init().unwrap();
    let opts = Opts::from_args();
    let res = submit(&opts.videos, &opts.cookie, &opts.vid, opts.title, &opts.tag, opts.cover).await;
    if opts.execution_summary_path.is_some() {
        gen_execution_summary(res, opts.execution_summary_path.unwrap());
    }
    Ok(())
}
