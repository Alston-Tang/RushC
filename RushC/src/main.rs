use biliup::uploader::bilibili::{BiliBili, Vid, Video};
use biliup::uploader::credential::login_by_cookies;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use std::time::Instant;
use log::info;
use biliup::client::StatelessClient;
use biliup::uploader::{line, VideoFile};
use bytes::{Buf, Bytes};
use indicatif::{ProgressBar, ProgressStyle};
use anyhow::{Context, Result};
use futures::{Stream, StreamExt};
use reqwest::Body;
use ftlog::appender::FileAppender;
use ftlog::{debug, trace};

#[derive(Clone)]
struct Progressbar {
    bytes: Bytes,
    pb: ProgressBar,
}

impl From<Progressbar> for Body {
    fn from(async_stream: Progressbar) -> Self {
        Body::wrap_stream(async_stream)
    }
}

impl Progressbar {
    pub fn new(bytes: Bytes, pb: ProgressBar) -> Self {
        Self { bytes, pb }
    }

    pub fn progress(&mut self) -> Result<Option<Bytes>> {
        let pb = &self.pb;

        let content_bytes = &mut self.bytes;

        let n = content_bytes.remaining();

        let pc = 4096;
        if n == 0 {
            Ok(None)
        } else if n < pc {
            pb.inc(n as u64);
            Ok(Some(content_bytes.copy_to_bytes(n)))
        } else {
            pb.inc(pc as u64);

            Ok(Some(content_bytes.copy_to_bytes(pc)))
        }
    }
}

impl Stream for Progressbar {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.progress()? {
            None => Poll::Ready(None),
            Some(s) => Poll::Ready(Some(Ok(s))),
        }
    }
}

pub async fn upload(
    video_path: &[PathBuf],
    bili: &BiliBili,
) -> Result<Vec<Video>> {
    let limit: usize = 10;
    info!("number of concurrent futures: {limit}");
    let mut videos = Vec::new();
    let client = StatelessClient::default();
    let line = line::qn();
    for video_path in video_path {
        info!("{line:?}");
        let video_file = VideoFile::new(video_path)
            .with_context(|| format!("file {}", video_path.to_string_lossy()))?;
        let total_size = video_file.total_size;
        let file_name = video_file.file_name.clone();
        let uploader = line.pre_upload(bili, video_file).await?;
        //Progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?);
        // pb.enable_steady_tick(Duration::from_secs(1));
        // pb.tick()

        let instant = Instant::now();

        let video = uploader
            .upload(client.clone(), limit, |vs| {
                vs.map(|chunk| {
                    let pb = pb.clone();
                    let chunk = chunk?;
                    let len = chunk.len();
                    Ok((Progressbar::new(chunk, pb), len))
                })
            })
            .await?;
        pb.finish_and_clear();
        let t = instant.elapsed().as_millis();
        info!(
            "Upload completed: {file_name} => cost {:.2}s, {:.2} MB/s.",
            t as f64 / 1000.,
            total_size as f64 / 1000. / t as f64
        );
        videos.push(video);
    }
    Ok(videos)
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let user_cookie_path = PathBuf::from("E:\\r46mht\\cookies.json");
    let bilibili = login_by_cookies(user_cookie_path).await.unwrap();
    let vid = Vid::Bvid(String::from("BV1z84y1U7tm"));
    let studio = bilibili.studio_data(&vid).await.unwrap();

    let video_path =
        PathBuf::from("D:\\Seafile\\Seafile\\thm64\\My Photos\\Camera\\VID_20230820_180055.mp4");
    let upload_res = upload(&[video_path], &bilibili).await.unwrap();
    print!("{:?}", upload_res);
    Ok(())
}
