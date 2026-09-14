use serde::Serialize;
use std::time::Instant;

use crate::models::Capabilities;

/// Result of probing one model. Fields omitted when a test was skipped.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProbeResult {
    pub model: String,
    pub caps: Option<Capabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping: Option<PingOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<LatencyOutcome>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PingOutcome {
    pub ok: bool,
    pub http_code: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct LatencyOutcome {
    pub ttft_secs: Option<f64>,
    pub total_secs: Option<f64>,
    pub tokens: u64,
    pub rate_per_sec: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ProbeOpts {
    pub base_url: String,
    pub api_key: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f64,
    pub timeout_secs: u64,
    pub do_ping: bool,
    pub do_latency: bool,
}

fn client(timeout: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

/// Build the OpenAI chat body for a model, requesting usage in stream if supported.
fn chat_body(
    model: &str,
    prompt: &str,
    max_tokens: u32,
    temp: f64,
    stream: bool,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "temperature": temp,
        "stream": stream,
    });
    if stream {
        body["stream_options"] = serde_json::json!({
            "include_usage": true
        });
    }
    body
}

/// Non-streaming ping: returns HTTP code (200 → ok).
async fn ping(host: &str, key: &str, opts: &ProbeOpts) -> Result<PingOutcome, String> {
    let c = client(opts.timeout_secs)?;
    let url = format!("{}/v1/chat/completions", opts.base_url);
    let resp = c
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .json(&chat_body(host, &opts.prompt, opts.max_tokens, opts.temperature, false))
        .send()
        .await
        .map_err(|e| format!("POST {url}: {e}"))?;
    let code = resp.status().as_u16();
    Ok(PingOutcome {
        ok: code == 200,
        http_code: code,
    })
}

/// Streaming latency: measures time-to-first-token (including reasoning/thinking) and total throughput.
/// Streams SSE lines, extracting usage.completion_tokens or estimating tokens from stream chunks.
async fn latency(host: &str, key: &str, opts: &ProbeOpts) -> Result<LatencyOutcome, String> {
    let c = client(opts.timeout_secs)?;
    let url = format!("{}/v1/chat/completions", opts.base_url);
    let started = Instant::now();
    let resp = c
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .json(&chat_body(host, &opts.prompt, opts.max_tokens, opts.temperature, true))
        .send()
        .await
        .map_err(|e| format!("POST {url}: {e}"))?;

    let connected = Instant::now();

    if !resp.status().is_success() {
        return Ok(LatencyOutcome {
            ttft_secs: None,
            total_secs: None,
            tokens: 0,
            rate_per_sec: None,
        });
    }

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let mut ttft: Option<f64> = None;
    let mut tokens: u64 = 0;
    let mut chunk_count: u64 = 0;
    let mut accumulated_chars: usize = 0;
    let mut stream_buffer = String::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(_) => break,
        };
        let chunk_str = String::from_utf8_lossy(&chunk);
        stream_buffer.push_str(&chunk_str);

        // Process complete SSE lines
        while let Some(newline_pos) = stream_buffer.find('\n') {
            let line = stream_buffer[..newline_pos].trim().to_string();
            stream_buffer = stream_buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if line == "data: [DONE]" {
                break;
            }

            if let Some(json_str) = line.strip_prefix("data:") {
                let json_str = json_str.trim();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    // 1. Extract usage if provided in the stream
                    if let Some(usage) = val.get("usage") {
                        if let Some(ct) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
                            if ct > 0 {
                                tokens = ct;
                            }
                        }
                    }

                    // 2. Detect first token for TTFT (supporting content, reasoning_content, reasoning, thinking)
                    if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                        if let Some(first_choice) = choices.first() {
                            if let Some(delta) = first_choice.get("delta") {
                                let mut text_found = false;

                                for field in &["content", "reasoning_content", "reasoning", "thinking"] {
                                    if let Some(s) = delta.get(*field).and_then(|v| v.as_str()) {
                                        if !s.is_empty() {
                                            text_found = true;
                                            accumulated_chars += s.len();
                                            chunk_count += 1;
                                            break;
                                        }
                                    }
                                }

                                if text_found && ttft.is_none() {
                                    ttft = Some(connected.elapsed().as_secs_f64());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let total = started.elapsed().as_secs_f64();

    // Fallback token count if usage chunk was not delivered by provider
    if tokens == 0 && chunk_count > 0 {
        tokens = chunk_count.max((accumulated_chars as u64 + 3) / 4);
    }

    let rate = if total > 0.0 && tokens > 0 {
        Some(tokens as f64 / total)
    } else {
        None
    };

    Ok(LatencyOutcome {
        ttft_secs: ttft,
        total_secs: Some(total),
        tokens,
        rate_per_sec: rate,
    })
}

/// Probe a single model.
pub async fn probe_one(model: &str, opts: &ProbeOpts) -> Result<ProbeResult, String> {
    let mut out = ProbeResult {
        model: model.to_string(),
        ..Default::default()
    };
    if opts.do_ping {
        out.ping = Some(ping(model, &opts.api_key, opts).await?);
    }
    if opts.do_latency {
        out.latency = Some(latency(model, &opts.api_key, opts).await?);
    }
    Ok(out)
}
