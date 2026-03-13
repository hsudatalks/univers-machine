use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteFileEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) size: u64,
    pub(crate) is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteDirectoryListing {
    pub(crate) target_id: String,
    pub(crate) path: String,
    pub(crate) parent_path: Option<String>,
    pub(crate) entries: Vec<RemoteFileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteFilePreview {
    pub(crate) target_id: String,
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) is_binary: bool,
    pub(crate) truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserScreenshotCapture {
    pub(crate) target_id: String,
    pub(crate) path: String,
}
