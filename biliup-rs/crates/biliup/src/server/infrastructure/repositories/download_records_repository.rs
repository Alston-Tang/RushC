use crate::server::core::live_streamers::{DownloadRecords, DownloadRecordsRepository};
use async_trait::async_trait;

#[derive(Clone)]
pub struct SqliteDownloadRecordsRepository {}

impl SqliteDownloadRecordsRepository {
    pub fn new() -> Self {
        Self {  }
    }
}

#[async_trait]
impl DownloadRecordsRepository for SqliteDownloadRecordsRepository {
    async fn create(&self, _entity: DownloadRecords) -> anyhow::Result<DownloadRecords> {
        todo!()
    }

    async fn get_all(&self) -> anyhow::Result<Vec<DownloadRecords>> {
        todo!()
    }
}
