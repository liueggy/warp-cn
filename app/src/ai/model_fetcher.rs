//! Model list fetcher for SSYCloud / OpenAI-compatible endpoints.
//!
//! Fetches available models from `GET {endpoint}/models` and returns them.
//! This module is ready for UI integration (e.g. model selection dropdown in settings).

use serde::Deserialize;

/// A single model entry from the /models API.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub owned_by: Option<String>,
    /// 胜算云返回的 support_apis 字段，每个模型支持的 API 端点列表
    /// 如 ["/v1/chat/completions", "/v1/messages", "/v1/responses"]
    #[serde(default)]
    pub support_apis: Vec<String>,
}

impl ModelInfo {
    /// 根据 support_apis 返回首选端点路径。
    /// 优先级: /v1/responses > /v1/chat/completions > /v1/messages > 第一个可用的。
    pub fn preferred_api(&self) -> &str {
        for preferred in &["/v1/responses", "/v1/chat/completions", "/v1/messages"] {
            if self.support_apis.iter().any(|a| a == preferred) {
                return preferred;
            }
        }
        self.support_apis
            .first()
            .map(String::as_str)
            .unwrap_or("/v1/chat/completions")
    }
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

/// Fetch the list of available models from an OpenAI-compatible endpoint.
pub async fn fetch_models(endpoint: &str, api_key: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/models", endpoint.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("获取模型列表失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body: String = resp.text().await.unwrap_or_default();
        return Err(format!("获取模型列表错误 ({}): {}", status.as_u16(), body));
    }

    let parsed: ModelsResponse = resp.json().await
        .map_err(|e| format!("解析模型列表失败: {}", e))?;

    let mut models = parsed.data;

    // Sort with priority models first
    sort_models(&mut models);

    // Fallback if empty
    if models.is_empty() {
        models = default_models();
    }

    Ok(models)
}

/// Sort models: put common models first for better UX.
fn sort_models(models: &mut [ModelInfo]) {
    let priority_order = [
        "gpt-4o", "gpt-4-turbo", "gpt-4", "claude", "gemini",
        "deepseek", "qwen", "glm", "llama",
    ];

    models.sort_by(|a, b| {
        let a_prio = priority_order.iter().position(|p| a.id.contains(p)).unwrap_or(usize::MAX);
        let b_prio = priority_order.iter().position(|p| b.id.contains(p)).unwrap_or(usize::MAX);
        a_prio.cmp(&b_prio).then_with(|| a.id.cmp(&b.id))
    });
}

/// Fallback default models list when API is unavailable.
fn default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo { id: "gpt-4o".into(), object: "model".into(), created: None, owned_by: Some("openai".into()), support_apis: vec!["/v1/chat/completions".into(), "/v1/messages".into()] },
        ModelInfo { id: "gpt-4o-mini".into(), object: "model".into(), created: None, owned_by: Some("openai".into()), support_apis: vec!["/v1/chat/completions".into(), "/v1/messages".into(), "/v1/responses".into()] },
        ModelInfo { id: "gpt-4-turbo".into(), object: "model".into(), created: None, owned_by: Some("openai".into()), support_apis: vec!["/v1/chat/completions".into(), "/v1/messages".into()] },
    ]
}
