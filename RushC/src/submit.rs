use anyhow::Error;
use biliup::uploader::bilibili::{BiliBili, ResponseData, Studio, Vid, Video};
use biliup::uploader::credential::login_by_cookies;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use structopt::StructOpt;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub struct VideoInfo {
    pub path: String,
    pub video_title: String,
}

impl FromStr for VideoInfo {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let video_info_split = value.split(":").collect::<Vec<&str>>();
        if video_info_split.len() != 2 {
            panic!("video info should be a [path:title] string but got {value}");
        }
        Ok(VideoInfo {
            path: video_info_split[0].to_string(),
            video_title: video_info_split[1].to_string(),
        })
    }
}

impl fmt::Debug for VideoInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VideoInfo")
            .field("path", &self.path)
            .field("video_title", &self.video_title)
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
    title: Option<String>,
    #[structopt(long)]
    tag: String,
    #[structopt(long)]
    cover: Option<PathBuf>,
    #[structopt(long)]
    source: Option<String>,
    #[structopt(long)]
    desc: String,
    #[structopt(long)]
    tid: u16,
    #[structopt(use_delimiter = true)]
    videos: Vec<VideoInfo>,
}

async fn build_archive_studio(
    bili: &BiliBili,
    vid: &Option<String>,
    title: &Option<String>,
    tag: &str,
    source: &Option<String>,
    desc: &String,
    tid: u16,
) -> Studio {
    let copyright = match source {
        Some(_) => 2,
        None => 1,
    };
    let studio = match vid {
        Some(vid_str) => {
            let mut exist_studio = bili.studio_data(&Vid::Bvid(vid_str.clone())).await.unwrap();
            if title.is_some() {
                exist_studio.title = title.clone().unwrap();
            }
            exist_studio
        }
        None => Studio {
            copyright,
            source: source.clone().unwrap_or(String::new()),
            tid: tid,
            cover: "".to_string(),
            title: title.clone().unwrap_or("".to_string()),
            desc_format_id: 0,
            desc: desc.clone(),
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
            title: Some(video.video_title.clone()),
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
    source: Option<String>,
    desc: String,
    tid: u16,
) -> SubmitResponse {
    info!("get user credential from cookie file");
    let bili = login_by_cookies(cookie).await.unwrap();
    info!(
        "user: {}",
        bili.my_info().await.unwrap()["data"]["name"]
            .as_str()
            .unwrap()
    );
    for (idx, video) in videos.iter().enumerate() {
        info!("video {}:", idx);
        info!("title= {}", video.video_title);
        info!("path= {}", video.path);
    }
    let mut studio = build_archive_studio(&bili, &vid, &title, &tag, &source, &desc, tid).await;
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

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();
    let opts = Opts::from_args();
    submit(
        &opts.videos,
        &opts.cookie,
        &opts.vid,
        opts.title,
        &opts.tag,
        opts.cover,
        opts.source,
        opts.desc,
        opts.tid,
    )
    .await;
    Ok(())
}
