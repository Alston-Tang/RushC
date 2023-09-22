use clap::Parser;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    password: String,
    #[arg(long)]
    base_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct FileRecord {
    pub relative_path: String,
    pub host_id: String,
    pub recorder_event_id: Option<ObjectId>,
}

#[derive(Serialize, Deserialize, Debug)]
struct GetFilesResponse {
    error: Option<String>,
    data: Vec<FileRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct DeleteFilesRequest {
    request: String,
    files_path: Vec<String>,
}

async fn get_files_list(base_url: &str, password: &str) -> Result<Vec<String>, ()> {
    let client = reqwest::Client::new();
    let url = String::from(base_url) + "/bililive_recorder/files";
    let request = client
        .get(url)
        .header("Authorization", format!("Bearer {}", password));
    let response = request.send().await.unwrap();
    if !response.status().is_success() {
        println!("{}", response.status());
        return Err(());
    }
    let files_records = response.json::<GetFilesResponse>().await.unwrap();
    if files_records.error.is_some() {
        println!("{}", files_records.error.unwrap());
        return Err(());
    }
    Ok(files_records
        .data
        .iter()
        .map(|record| record.relative_path.clone())
        .collect())
}

async fn delete_files(base_url: &str, password: &str, files: Vec<String>) -> Result<(), ()> {
    let client = reqwest::Client::new();
    let url = String::from(base_url) + "/bililive_recorder/files";
    let body = DeleteFilesRequest {
        request: String::from("DeleteFiles"),
        files_path: files,
    };
    let request = client
        .post(url)
        .header("Authorization", format!("Bearer {}", password))
        .json::<DeleteFilesRequest>(&body);
    let response = request.send().await.unwrap();
    if !response.status().is_success() {
        println!("{}", response.status());
        return Err(());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Cli = Parser::parse();
    let files = get_files_list(&*args.base_url, &args.password)
        .await
        .unwrap();
    println!("{:?}", files);
    delete_files(&*args.base_url, &args.password, files)
        .await
        .unwrap();
    Ok(())
}
