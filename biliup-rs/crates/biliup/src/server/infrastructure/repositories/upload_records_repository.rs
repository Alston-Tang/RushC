use crate::server::core::upload_streamers::{UploadRecords, UploadRecordsRepository};
use async_trait::async_trait;

#[derive(Clone)]
pub struct SqliteUploadRecordsRepository {}

impl SqliteUploadRecordsRepository {
    pub fn new() -> Self {
        Self { }
    }
}

#[async_trait]
impl UploadRecordsRepository for SqliteUploadRecordsRepository {
    async fn create(&self, _entity: UploadRecords) -> anyhow::Result<UploadRecords> {
        todo!()
    }

    async fn delete(&self, _id: i64) -> anyhow::Result<()> {
        todo!()
    }

    async fn get_all(&self) -> anyhow::Result<Vec<UploadRecords>> {
        todo!()
    }

    async fn get_by_id(&self, _id: i64) -> anyhow::Result<UploadRecords> {
        todo!()
    }
}
