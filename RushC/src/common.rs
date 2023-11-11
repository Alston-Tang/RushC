use anyhow::Result;
use bytes::{Buf, Bytes};
use futures::Stream;
use log::info;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::{Client, Collection};
use reqwest::Body;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::str::FromStr;
use std::task::Poll;
use serde::de::DeserializeOwned;

// this structure should be consistent to structure definition in MultiverseConverge
// class AirflowVideoLock(Document):
//     DocId = ObjectIdField(require=True)
//     LockId = ObjectIdField(required=False, default=None)
//
//     meta = {"db_alias": "airflow", "indexes": [{'fields': ['DocId'], 'unique': True}]}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AirflowVideoLock {
    doc_id: ObjectId,
    lock_id: ObjectId,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AirflowArchiveLock {
    doc_id: ObjectId,
    lock_id: ObjectId,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct BiliVideoInfo {
    pub doc_id: ObjectId,
    pub title: Option<String>,
    pub filename: String,
    pub desc: String,
    pub bvid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct BiliArchiveInfo {
    pub archive_id: ObjectId,
    pub aid: i64,
    pub bvid: String,
}

#[derive(Clone)]
pub struct PollStream {
    bytes: Bytes,
}

impl PollStream {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn step(&mut self) -> Result<Option<Bytes>> {
        let content_bytes = &mut self.bytes;
        let remaining = content_bytes.remaining();
        let pc = 4096;
        let n = if remaining > pc { pc } else { remaining };
        if n == 0 {
            Ok(None)
        } else {
            Ok(Some(content_bytes.copy_to_bytes(n)))
        }
    }
}

impl Stream for PollStream {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.step()? {
            None => Poll::Ready(None),
            Some(s) => Poll::Ready(Some(Ok(s))),
        }
    }
}

impl From<PollStream> for Body {
    fn from(async_stream: PollStream) -> Self {
        Body::wrap_stream(async_stream)
    }
}

pub async fn get_mongodb_client(mongodb_uri: &str) -> Client {
    info!("connecting to mongodb {mongodb_uri}");
    Client::with_uri_str(mongodb_uri).await.unwrap()
}

pub async fn check_lock_acquired<DocDef>(
    collection: &Collection<DocDef>,
    lock_id: &str,
    file_obj_id: &str,
) -> bool
where
    DocDef: Serialize + DeserializeOwned + Unpin + Send + Sync,
{
    let res = collection
        .find_one(doc! {"LockId": ObjectId::from_str(lock_id).unwrap(), "DocId": ObjectId::from_str(file_obj_id).unwrap()}, None)
        .await
        .unwrap()
        .is_some();
    info!("lock for {file_obj_id} has been acquired by {lock_id}? {res}");
    res
}
