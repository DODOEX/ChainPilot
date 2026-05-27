use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: Option<String>,
    pub masked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigStatus {
    pub key: String,
    pub action: String,
    pub message: String,
}
