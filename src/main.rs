use std::io::{Write, stdin, stdout};

use anyhow::{Context, Ok, Result};
use futures_util::StreamExt;
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
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Delta {
    content: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut message_history: Vec<Message> = vec![Message {
        role: "system".to_string(),
        content: "You are a helpful assistant.".to_string(),
    }];
    println!("你好，有什么可以帮助你的?");

    loop {
        print!("\n你: ");
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
        // 流输出
        let (reply_message, reply_usage) = request_to_response_as_stream(&message_history).await?;

        println!(
            "\n本次prompt_tokens: {},本次completion_tokens: {}, 本次total_tokens: {}",
            reply_usage.prompt_tokens, reply_usage.completion_tokens, reply_usage.total_tokens
        );

        message_history.push(reply_message);
    }
}

async fn request_to_response_as_stream(messages: &[Message]) -> Result<(Message, Usage)> {
    let url = "https://api.deepseek.com/chat/completions";
    let token = std::env::var("DEEPSEEK_API_KEY").context("获取API KEY 失败")?;

    let chat_request = ChatRequest {
        model: "deepseek-flash".to_string(),
        messages: messages.to_vec(),
        thinking: Thinking {
            kind: "disabled".to_string(),
        },
        stream: true,
    };

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&chat_request)
        .send()
        .await
        .context("发送HTTP请求失败")?;

    let mut body = response.bytes_stream();

    let mut buf: Vec<u8> = Vec::new();

    let mut content = String::new();

    let mut usage = Usage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };

    println!();
    'outer: while let Some(item) = body.next().await {
        buf.extend(item?.iter());
        //buf.extend_from_slice(&item?);

        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line = std::str::from_utf8(&buf[..pos])?.to_string();
            buf.drain(..=pos);

            if line.trim().is_empty() {
                continue;
            } else if let Some(l) = line.strip_prefix("data: ") {
                if l == "[DONE]" {
                    break 'outer;
                } else {
                    let parsed: StreamChunk = serde_json::from_str(l)?;
                    if let Some(par) = parsed.choices.first() {
                        if let Some(ref s) = par.delta.content {
                            let mut out = stdout();
                            print!("{}", s);
                            content.push_str(s);
                            out.flush()?;
                        }
                    }

                    if let Some(us) = parsed.usage {
                        usage = us
                    }
                }
            }
        }
    }
    println!();

    let message_reply = Message {
        role: "assistant".to_string(),
        content: content,
    };
    Ok((message_reply, usage))
}
