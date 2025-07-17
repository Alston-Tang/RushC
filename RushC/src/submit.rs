use anyhow::Error;
use biliup::uploader::bilibili::{BiliBili, ResponseData, Studio, Vid, Video};
use biliup::uploader::credential::login_by_cookies;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Serialize, Deserialize)]
struct VideoInfo {
    path: String,
    video_title: String,
}

#[derive(Serialize, Deserialize)]
struct SubmitConfig {
    vid: Option<String>,
    cookie: PathBuf,
    title: Option<String>,
    tag: Option<Vec<String>>,
    cover: Option<PathBuf>,
    source: Option<String>,
    desc: Option<String>,
    tid: Option<u16>,
    videos: Vec<VideoInfo>
}

fn parse_config(config_path: &Path) -> Result<SubmitConfig, Error> {
    let config_file = File::open(config_path)?;
    let reader = BufReader::new(config_file);

    let config: SubmitConfig = serde_json::from_reader(reader)?;
    info!("{}", serde_json::to_string(&config).unwrap());

    Ok(config)
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
    #[structopt(parse(from_os_str), long)]
    config: PathBuf,
    #[structopt(parse(from_os_str), long)]
    out: Option<PathBuf>
}

async fn build_archive_studio(
    bili: &BiliBili,
    config: &SubmitConfig,
) -> Studio {
    let mut studio = match &config.vid {
        Some(vid) => {
            info!("vid {} provided. get existing studio from remote", vid);
            bili.studio_data(&Vid::Bvid(vid.clone()), None).await.unwrap()
        },
        None => {
            info!("create a default Studio struct");
            Studio {
                copyright: 0,
                source: "".to_string(),
                tid: 0,
                cover: "".to_string(),
                title: "".to_string(),
                desc_format_id: 0,
                desc: "".to_string(),
                desc_v2: None,
                dynamic: "".to_string(),
                subtitle: Default::default(),
                tag: "".to_string(),
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
                extra_fields: None
            }
        }
    };

    // override if fields present in config
    studio.copyright = match config.source {
        Some(_) => 2,
        None => 1,
    };
    studio.source = config.source.clone().unwrap_or(studio.source);
    // 动画-综合 = 27
    studio.tid = config.tid.unwrap_or(studio.tid);
    studio.cover = match &config.cover {
        Some(path) => cover_up(&bili, path.clone()).await,
        None => studio.cover
    };
    studio.title = config.title.clone().unwrap_or(studio.title);
    studio.desc = config.desc.clone().unwrap_or(studio.desc);
    studio.tag = match &config.tag {
        Some(tag) => tag.join(","),
        None => studio.tag
    };
    studio.videos =  construct_videos_list(&config.videos);

    studio
}

pub async fn cover_up(bili: &BiliBili, cover: PathBuf) -> String {
    let url = bili.cover_up(&std::fs::read(cover).unwrap()).await.unwrap();
    info!("cover is uploaded to {url}");
    url
}

fn construct_videos_list(videos: &Vec<VideoInfo>) -> Vec<Video> {
    videos
        .iter()
        .map(|video| Video {
            title: Some(video.video_title.clone()),
            filename: video.path.clone(),
            desc: String::new(),
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

async fn submit(config: SubmitConfig) -> SubmitResponse {
    info!("get user credential from cookie file");
    let bili = login_by_cookies(&config.cookie, None).await.unwrap();
    info!(
        "user: {}",
        bili.my_info().await.unwrap()["data"]["name"]
            .as_str()
            .unwrap()
    );
    let studio = build_archive_studio(&bili, &config).await;
    info!("studio: {:?}", studio);
    match config.vid {
        Some(vid) => {
            info!("editing existing archive {}", vid);
            let res = bili.edit(&studio, None).await.unwrap();
            info!("{:?}", res);
            parse_edit_response(res)
        },
        None => {
            info!("adding a new archive");
            let res = bili.submit(&studio, None).await.unwrap();
            info!("{:?}", res);
            parse_submit_response(res)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();
    let opts = Opts::from_args();

    info!("using config {}", opts.config.display());
    let config = parse_config(&opts.config)?;

    let res = submit(config).await;

    match opts.out {
        Some(path) => {
            let file = OpenOptions::new().write(true).create(true).open(path)?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &res)?;
        },
        None => {},
    }
    Ok(())
}
