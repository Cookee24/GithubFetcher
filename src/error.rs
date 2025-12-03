use std::collections::HashMap;

use rmcp::model::Content;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ApiErrorBody {
    pub message: String,
    pub code: String,
}

impl ApiErrorBody {
    pub fn new(message: impl Into<String>, code: impl ToString) -> Self {
        Self {
            message: message.into(),
            code: code.to_string(),
        }
    }

    pub fn from_reqwest(err: reqwest::Error) -> Self {
        let code = err
            .status()
            .map(|s| s.as_u16().to_string())
            .unwrap_or_else(|| "0".to_string());
        Self::new(err.to_string(), code)
    }

    pub async fn from_response(status: reqwest::StatusCode, response: reqwest::Response) -> Self {
        let body = response.text().await.unwrap_or_default();
        let fallback = status
            .canonical_reason()
            .unwrap_or("GitHub API error")
            .to_string();

        let message = serde_json::from_str::<HashMap<String, serde_json::Value>>(&body)
            .ok()
            .and_then(|map| {
                map.get("message")
                    .and_then(|m| m.as_str().map(str::to_string))
            })
            .unwrap_or_else(|| {
                if body.is_empty() {
                    fallback
                } else {
                    body.clone()
                }
            });

        Self::new(message, status.as_u16().to_string())
    }
}

impl rmcp::model::IntoContents for ApiErrorBody {
    fn into_contents(self) -> Vec<Content> {
        Content::json(&self)
            .map(|content| vec![content])
            .unwrap_or_else(|_| vec![Content::text(format!("{} ({})", self.message, self.code))])
    }
}
