use futures::stream::StreamExt;
use mongodb::bson::document::Document;
use mongodb::bson::oid::ObjectId;
use mongodb::results::InsertManyResult;
use mongodb::{
    bson::doc,
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Collection, Database,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use tokio;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    // path prefix of all recorder files
    #[arg(long)]
    files_root: std::path::PathBuf,
    // id uniquely identified a host
    #[arg(long)]
    host_id: String,
    // mongodb_uri
    #[arg(long)]
    mongodb_uri: String,
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct FileRecord {
    pub relative_path: String,
    pub host_id: String,
    pub recorder_event_id: Option<ObjectId>,
}

async fn get_all_file_closed_events(db: &Database) -> Result<Vec<Document>, String> {
    let collection: Collection<Document> = db.collection("records");
    let mut event_cursor = collection
        .find(doc! {"EventType": "FileClosed"}, None)
        .await
        .unwrap();
    let mut res = Vec::<Document>::new();
    while let Some(doc) = event_cursor.next().await {
        res.push(doc.unwrap())
    }
    Ok(res)
}

fn scan_recording_files(path: &Path) -> Vec<PathBuf> {
    let mut res = Vec::new();
    for item in read_dir(path).unwrap() {
        let file_item = item.unwrap();
        if file_item.file_type().unwrap().is_dir() {
            let file_path = file_item.path();
            let mut files = scan_recording_files(file_path.as_path());
            res.append(&mut files)
        }
        if !file_item.file_type().unwrap().is_file() {
            continue;
        }
        let file_path = file_item.path();
        if file_path.extension().unwrap().to_ascii_lowercase() != "flv" {
            continue;
        }
        res.push(file_path);
    }
    res
}

async fn connect_to_db(uri: &str) -> Result<Database, ()> {
    let mut client_options = ClientOptions::parse(uri).await.unwrap();

    // Set the server_api field of the client_options object to Stable API version 1
    let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
    client_options.server_api = Some(server_api);
    // Create a new client and connect to the server
    let client = Client::with_options(client_options).unwrap();

    Ok(client.database("bililive"))
}

fn build_file_obj_id_lookup_map(file_closed_events: &Vec<Document>) -> HashMap<String, ObjectId> {
    HashMap::from_iter(file_closed_events.iter().map(|doc: &Document| {
        (
            String::from(
                doc.get_document("EventData")
                    .unwrap()
                    .get_str("RelativePath")
                    .unwrap(),
            ),
            doc.get_object_id("_id").unwrap(),
        )
    }))
}

async fn insert_file_record(
    db: &Database,
    files: Vec<String>,
    host_id: &str,
    file_closed_events: &Vec<Document>,
) -> InsertManyResult {
    let file_obj_id_lookup = build_file_obj_id_lookup_map(file_closed_events);

    for (key, value) in &file_obj_id_lookup {
        println!("{}", key);
    }
    let collection = db.collection::<FileRecord>("files");
    let file_records = files.iter().map(|path: &String| {
        let recorder_event_id = file_obj_id_lookup.get(path);
        FileRecord {
            host_id: String::from(host_id),
            relative_path: String::from(path),
            recorder_event_id: match recorder_event_id {
                Some(ObjectId) => Some(recorder_event_id.unwrap().clone()),
                None => None,
            },
        }
    });
    collection.insert_many(file_records, None).await.unwrap()
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Cli = Parser::parse();
    println!("host_id={}, files_root={}, mongodb_uri={}", args.host_id, args.files_root.to_str().unwrap(), args.mongodb_uri);
    let db_future = connect_to_db(args.mongodb_uri.as_str());
    let root_path: &Path = args.files_root.as_path();
    let files: Vec<String> = scan_recording_files(root_path)
        .iter()
        .map(|path: &PathBuf| {
            let relative_path =
                PathBuf::from(path.strip_prefix(root_path).unwrap());
            relative_path.to_str().unwrap().replace("\\", "/")
        })
        .collect();

    let db = db_future.await.unwrap();
    let file_closed_events = get_all_file_closed_events(&db).await.unwrap();
    let res = insert_file_record(&db, files, args.host_id.as_str(), &file_closed_events).await;
    println!("{}", res.inserted_ids.len());
    Ok(())
}
