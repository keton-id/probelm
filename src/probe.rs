use serde::Serialize;
use std::time::Instant;

use crate::models::Capabilities;

/// Result of probing one model. Phone fields omitted when a test was skipped.
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

/// Build the OpenAI chat body for a model.
fn chat_body(model: &str, prompt: &str, max_tokens: u32, temp: f64, stream: bool) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "temperature": temp,
        "stream": stream,
    })
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
    Ok(PingOutcome { ok: code == 200, http_code: code })
}

/// Streaming latency: measures time-to-first-token and total time.
/// Streams SSE lines, counting completion tokens from the final usage chunk.
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
    if !resp.status().is_success() {
        return Ok(LatencyOutcome { ttft_secs: None, total_secs: None, tokens: 0, rate_per_sec: None });
    }

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let mut ttft: Option<f64> = None;
    let mut tokens: u64 = 0;
    let mut buf = String::new();
    loop {
        match stream.next().await {
            Some(Ok(chunk)) => {
                let data = String::from_utf8_lossy(&chunk);
                // TTFT: first chunk carrying non-empty content in a delta
                if ttft.is_none() && data.contains("\"delta\":") && data.contains("\"content\":\"") {
                    let j: serde_json::Value = match serde_json::from_str(
                        &data.trim_start_matches("data:").trim(),
                    ) {
                        Ok(v) => v,
                        Err(_) => serde_json::Value::Null,
                    };
                    let has_text = j
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|a| a.first())
                        .and_then(|x| x.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                        .map(|s| !s.is_empty())
                        .unwrap_or(false);
                    if has_text {
                        ttft = Some(started.elapsed().as_secs_f64());
                    }
                }
                buf.push_str(&data);
                // extract usage.completion_tokens (final chunk)
                while let Some(idx) = buf.find("\"completion_tokens\":") {
                    let rest = &buf[idx + "\"completion_tokens\":".len()..];
                    let end = rest.find([',', '}']).unwrap_or(rest.len());
                    if let Ok(v) = rest[..end].trim().parse::<u64>() {
                        tokens = v;
                    }
                    buf = rest[end..].to_string();
                }
            }
            Some(Err(_)) => break,
            None => break,
        }
    }
    let total = started.elapsed().as_secs_f64();
    let rate = if total > 0.0 { Some(tokens as f64 / total) } else { None };
    Ok(LatencyOutcome { ttft_secs: ttft, total_secs: Some(total), tokens, rate_per_sec: rate })
}

/// Probe a single model.
pub async fn probe_one(model: &str, opts: &ProbeOpts) -> Result<ProbeResult, String> {
    let mut out = ProbeResult { model: model.to_string(), ..Default::default() };
    if opts.do_ping {
        out.ping = Some(ping(model, &opts.api_key, opts).await?);
    }
    if opts.do_latency {
        out.latency = Some(latency(model, &opts.api_key, opts).await?);
    }
    Ok(out)
}
