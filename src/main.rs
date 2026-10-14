use std::io::{Write, stdin, stdout};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Thinking {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    thinking: Thinking,
    stream: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatResponse {
    id: String,
    usage: Usage,
    choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Choice {
    index: u32,
    message: Message,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut message_history: Vec<Message> = vec![Message {
        role: "system".to_string(),
        content: "You are a helpful assistant.".to_string(),
    }];
    println!("你好，有什么可以帮助你的?");

    loop {
        print!("你: ");
        stdout().flush()?;
        let mut s = String::new();
        stdin().read_line(&mut s).context("读取标准输入失败")?;

        let input = s.trim().to_string();

        if input == "/exit" {
            return Ok(());
        }

        if input == "/clear" {
            message_history = vec![Message {
                role: "system".to_string(),
                content: "You are a helpful assistant.".to_string(),
            }];
            continue;
        }

        if input.is_empty() {
            continue;
        }

        let the_first = Message {
            role: "user".to_string(),
            content: input,
        };

        message_history.push(the_first);

        let (reply_message, reply_usage) = request_to_response(&message_history).await?;
        println!(
            "{} \n本次prompt_tokens: {},本次completion_tokens: {}, 本次total_tokens: {}",
            reply_message.content,
            reply_usage.prompt_tokens,
            reply_usage.completion_tokens,
            reply_usage.total_tokens
        );

        message_history.push(reply_message);
    }
}

async fn request_to_response(messages: &[Message]) -> Result<(Message, Usage)> {
    let url = "https://api.deepseek.com/chat/completions";
    let token = std::env::var("DEEPSEEK_API_KEY").context("获取API KEY 失败")?;

    let chat_request = ChatRequest {
        model: "deepseek-flash".to_string(),
        messages: messages.to_vec(),
        thinking: Thinking {
            kind: "disabled".to_string(),
        },
        stream: false,
    };

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&chat_request)
        .send()
        .await
        .context("发送HTTP请求失败")?;

    // 状态码
    let status = response.status();
    let body = response.text().await.context("读取响应 body 失败")?;

    if !status.is_success() {
        anyhow::bail!("API 返回错误状态 {},响应体: {}", status, body);
    }

    let chat_response: ChatResponse = serde_json::from_str(&body)
        .with_context(|| format!("解析 JSON 失败，原始响应：{}", body))?;

    let ChatResponse { usage, choices, .. } = chat_response;

    let reply_messages = choices
        .into_iter()
        .next()
        .context("响应中没有 choices")?
        .message;

    Ok((reply_messages, usage))
}
