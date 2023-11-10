use crate::server::core::live_streamers::{Videos, VideosRepository};
use async_trait::async_trait;

#[derive(Clone)]
pub struct SqliteVideosRepository {}

impl SqliteVideosRepository {
    pub fn new() -> Self {
        Self {  }
    }
}

#[async_trait]
impl VideosRepository for SqliteVideosRepository {
    async fn create(&self, _entity: Videos) -> anyhow::Result<Videos> {
        todo!()
    }

    async fn update(&self, _entity: Videos) -> anyhow::Result<Videos> {
        todo!()
    }

    async fn get_by_id(&self, _id: i64) -> anyhow::Result<Videos> {
        todo!()
    }
}
