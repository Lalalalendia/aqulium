use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileItemType {
    File,
    Folder,
}

#[derive(Debug, Serialize)]
pub struct FileItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: FileItemType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub content: String,
    pub hash: String,
    pub text_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub byte_length: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWriteResult {
    pub hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRenameResult {
    pub content: String,
    pub hash: String,
    pub text_hash: String,
    pub updated_paths: Vec<String>,
}
