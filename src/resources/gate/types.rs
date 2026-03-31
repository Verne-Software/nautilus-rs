#[derive(Debug, Clone, serde::Deserialize)]
pub struct Identity {
    pub id: String,
    pub schema_id: String,
    pub state: String,
    pub traits: IdentityTraits,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityTraits {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityTraitsInput {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateIdentityParams {
    pub schema_id: String,
    pub traits: IdentityTraitsInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonPatchOp {
    pub op: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccessToken {
    pub access_token: String,
    pub expires_at: String,
    pub subject: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateTokenParams {
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenInfo {
    pub active: bool,
    pub subject: String,
    pub tenant_id: String,
    pub scopes: Vec<String>,
    pub expires_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthorizeParams {
    pub subject: String,
    pub action: String,
    pub resource: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthorizationDecision {
    pub allowed: bool,
    pub decision_id: String,
    pub reason: String,
}
