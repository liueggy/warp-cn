//! OpenAI-compatible HTTP client for AI requests.
//!
//! When SSYCloud or a custom OpenAI-compatible endpoint is configured,
//! this module handles converting Warp's internal request format to
//! OpenAI chat completions API calls and streaming responses back as
//! `warp_multi_agent_api::ResponseEvent` events.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp_multi_agent_api as api;

use crate::server::server_api::AIApiError;

// ── OpenAI API types ──────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[serde(default)]
    message: Option<AssistantMessage>,
    #[serde(default)]
    delta: Option<Delta>,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Build an OpenAI chat completion request and stream the response as
/// `warp_multi_agent_api::ResponseEvent` events.
pub async fn stream_chat_completion(
    user_message: &str,
    model: &str,
    api_key: &str,
    endpoint: &str,
    _conversation_id: Option<&str>,
) -> Result<
    impl Stream<Item = Result<api::ResponseEvent, Arc<AIApiError>>> + Send,
    Arc<AIApiError>,
> {
    let request_id = Uuid::new_v4().to_string();
    let conversation_id = _conversation_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let task_id = Uuid::new_v4().to_string();
    let message_id = Uuid::new_v4().to_string();

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: user_message.to_string(),
        },
    ];

    let request_body = ChatCompletionRequest {
        model,
        messages,
        stream: false,
        max_tokens: Some(8192),
        temperature: None,
    };

    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            let err_msg = format!("AI 请求失败: {}", e);
            Arc::new(AIApiError::Other(anyhow::anyhow!(err_msg)))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body: String = response.text().await.unwrap_or_default();
        return Err(Arc::new(AIApiError::Other(anyhow::anyhow!(
            "AI 服务返回错误 ({}): {}",
            status.as_u16(),
            body
        ))));
    }

    let body: ChatCompletionResponse = response.json().await.map_err(|e| {
        Arc::new(AIApiError::Other(anyhow::anyhow!(
            "解析 AI 响应失败: {}",
            e
        )))
    })?;

    let content = body
        .choices
        .first()
        .and_then(|c| {
            c.message
                .as_ref()
                .and_then(|m| m.content.as_deref())
                .or_else(|| c.delta.as_ref().and_then(|d| d.content.as_deref()))
        })
        .unwrap_or("（AI 未返回内容）");

    // Build the response events
    let now = prost_types::Timestamp {
        seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        nanos: 0,
    };

    let stream_init = api::ResponseEvent {
        r#type: Some(api::response_event::Type::Init(
            api::response_event::StreamInit {
                conversation_id: conversation_id.clone(),
                request_id: request_id.clone(),
                run_id: Uuid::new_v4().to_string(),
            },
        )),
    };

    let agent_message = api::Message {
        id: message_id.clone(),
        task_id: task_id.clone(),
        request_id: request_id.clone(),
        timestamp: Some(now.clone()),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: content.to_string(),
            },
        )),
    };

    let task = api::Task {
        id: task_id.clone(),
        description: "AI 助手回复".into(),
        dependencies: None,
        messages: vec![agent_message],
        summary: String::new(),
        server_data: String::new(),
    };

    let client_action = api::ClientAction {
        action: Some(api::client_action::Action::CreateTask(
            api::client_action::CreateTask {
                task: Some(task),
            },
        )),
    };

    let client_actions_event = api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![client_action],
            },
        )),
    };

    let stream_finished = api::ResponseEvent {
        r#type: Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                token_usage: vec![],
                should_refresh_model_config: false,
                request_cost: None,
                conversation_usage_metadata: None,
                reason: Some(api::response_event::stream_finished::Reason::Done(
                    api::response_event::stream_finished::Done {},
                )),
            },
        )),
    };

    let events: Vec<Result<api::ResponseEvent, Arc<AIApiError>>> = vec![
        Ok(stream_init),
        Ok(client_actions_event),
        Ok(stream_finished),
    ];

    Ok(futures::stream::iter(events))
}

/// Extract the user's message text from a Warp multi-agent Request.
pub fn extract_user_message(request: &api::Request) -> Option<String> {
    use api::request::input::Type;
    use api::request::input::user_inputs::user_input::Input as UserInputType;

    let input = request.input.as_ref()?;
    match &input.r#type {
        Some(Type::UserInputs(user_inputs)) => {
            for ui in &user_inputs.inputs {
                if let Some(UserInputType::UserQuery(query)) = &ui.input {
                    if !query.query.is_empty() {
                        return Some(query.query.clone());
                    }
                }
            }
            None
        }
        Some(Type::QueryWithCannedResponse(q)) => Some(q.query.clone()),
        _ => None,
    }
}

/// System prompt used when routing to SSYCloud / custom OpenAI endpoints.
const SYSTEM_PROMPT: &str = "\
You are Warp AI, a helpful terminal assistant integrated into the Warp terminal emulator.

Your capabilities:
- Help users with shell commands, programming, debugging, and system administration
- Answer questions about command-line tools, git, docker, and development workflows
- Explain code and error messages
- Suggest terminal commands and workflows

Guidelines:
- Be concise and practical - users are in a terminal environment
- When suggesting commands, explain what they do briefly
- Use code blocks for commands and code snippets
- If you're unsure, say so rather than guessing

请用中文回答用户的问题。\
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_message_from_user_inputs() {
        let request = api::Request {
            input: Some(api::request::Input {
                context: None,
                r#type: Some(api::request::input::Type::UserInputs(
                    api::request::input::UserInputs {
                        inputs: vec![api::request::input::user_inputs::UserInput {
                            text: Some("如何查看磁盘空间?".to_string()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )),
            }),
            ..Default::default()
        };
        assert_eq!(
            extract_user_message(&request),
            Some("如何查看磁盘空间?".to_string())
        );
    }

    #[test]
    fn test_extract_user_message_none() {
        let request = api::Request {
            input: None,
            ..Default::default()
        };
        assert_eq!(extract_user_message(&request), None);
    }
}
