#[derive(Debug, Clone, serde::Deserialize)]
pub struct Message {
    pub id: String,
    pub event_type: String,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SendMessageParams {
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ListMessagesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
}
