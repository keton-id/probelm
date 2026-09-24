use std::time::Instant;

pub use probelm_proto::{
    Capabilities, LatencyOutcome, PingOutcome, ProbeError, ProbeOpts, ProbeResult, ProbeState,
};

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
    reasoning_effort: Option<&str>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "temperature": temp,
        "stream": stream,
    });
    if let Some(effort) = reasoning_effort {
        body["reasoning_effort"] = serde_json::json!(effort);
    }
    if stream {
        body["stream_options"] = serde_json::json!({
            "include_usage": true
        });
    }
    body
}

fn classify_http_error(code: u16, body: Option<&str>) -> ProbeError {
    let reason = body.unwrap_or("unknown").to_string();
    if code == 401 || code == 403 || code == 429 {
        ProbeError::Refused {
            reason: format!("HTTP {code}: {reason}"),
        }
    } else if code >= 500 {
        ProbeError::Unreachable {
            reason: format!("HTTP {code}: {reason}"),
        }
    } else {
        ProbeError::Malformed {
            reason: format!("HTTP {code}: {reason}"),
        }
    }
}

fn classify_request_error(error: reqwest::Error) -> ProbeError {
    let reason = error.to_string();
    if error.is_timeout() {
        ProbeError::Timeout { reason }
    } else if error.is_connect() || error.is_request() {
        ProbeError::Unreachable { reason }
    } else {
        ProbeError::Other { reason }
    }
}

/// Non-streaming ping: returns HTTP code (200 → ok).
async fn ping(
    host: &str,
    key: &str,
    opts: &ProbeOpts,
) -> (Option<PingOutcome>, Option<ProbeError>) {
    let c = match client(opts.timeout_secs) {
        Ok(c) => c,
        Err(reason) => {
            return (None, Some(ProbeError::Other { reason }));
        }
    };
    let url = format!("{}/v1/chat/completions", opts.base_url);
    let resp = match c
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .json(&chat_body(
            host,
            &opts.prompt,
            opts.max_tokens,
            opts.temperature,
            false,
            opts.reasoning_effort.as_deref(),
        ))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (None, Some(classify_request_error(e)));
        }
    };
    let code = resp.status().as_u16();
    if code == 200 {
        (
            Some(PingOutcome {
                ok: true,
                http_code: code,
            }),
            None,
        )
    } else {
        let body = resp.text().await.unwrap_or_default();
        let err = classify_http_error(code, Some(&body));
        (
            Some(PingOutcome {
                ok: false,
                http_code: code,
            }),
            Some(err),
        )
    }
}

/// Streaming latency: measures time-to-first-token (including reasoning/thinking) and total throughput.
/// Streams SSE lines, extracting usage.completion_tokens or estimating tokens from stream chunks.
async fn latency(
    host: &str,
    key: &str,
    opts: &ProbeOpts,
) -> (Option<LatencyOutcome>, Option<ProbeError>) {
    let c = match client(opts.timeout_secs) {
        Ok(c) => c,
        Err(reason) => {
            return (None, Some(ProbeError::Other { reason }));
        }
    };
    let url = format!("{}/v1/chat/completions", opts.base_url);
    let started = Instant::now();
    let resp = match c
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .json(&chat_body(
            host,
            &opts.prompt,
            opts.max_tokens,
            opts.temperature,
            true,
            opts.reasoning_effort.as_deref(),
        ))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (None, Some(classify_request_error(e)));
        }
    };

    let connected = Instant::now();

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let err = classify_http_error(code, Some(&body));
        return (
            Some(LatencyOutcome {
                ttft_secs: None,
                total_secs: None,
                tokens: 0,
                rate_per_sec: None,
            }),
            Some(err),
        );
    }

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let mut parser = SseParser::default();
    let mut ttft: Option<f64> = None;

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(_) => break,
        };
        parser.push(&String::from_utf8_lossy(&chunk));
        if parser.ttft_content && ttft.is_none() {
            ttft = Some(connected.elapsed().as_secs_f64());
        }
        if parser.done {
            break;
        }
    }

    let total = started.elapsed().as_secs_f64();

    // Fallback token count if usage chunk was not delivered by provider
    let mut tokens = parser.tokens;
    if tokens == 0 && parser.chunk_count > 0 {
        tokens = parser
            .chunk_count
            .max((parser.accumulated_chars as u64).div_ceil(4));
    }

    // Fair throughput: tokens divided by generation streaming time (total - pure TTFT)
    let gen_duration = match ttft {
        Some(t) if total > t => (total - t).max(0.001),
        _ => total.max(0.001),
    };

    let rate = if gen_duration > 0.0 && tokens > 0 {
        Some(tokens as f64 / gen_duration)
    } else {
        None
    };

    (
        Some(LatencyOutcome {
            ttft_secs: ttft,
            total_secs: Some(total),
            tokens,
            rate_per_sec: rate,
        }),
        None,
    )
}

/// Probe a single model.
///
/// Always returns a [`ProbeResult`]. Partial or total failures are represented
/// inside the result (e.g. `ping.ok = false` or `latency = None`) rather than
/// as an `Err`. The caller can inspect [`ProbeResult::state`] or the optional
/// error details to decide what to do.
pub async fn probe_one(model: &str, opts: &ProbeOpts) -> ProbeResult {
    let mut out = ProbeResult {
        model: model.to_string(),
        ..Default::default()
    };
    if opts.do_ping {
        let (ping, err) = ping(model, &opts.api_key, opts).await;
        out.ping = ping;
        if let Some(err) = err {
            out.ping = out.ping.or(Some(PingOutcome {
                ok: false,
                http_code: 0,
            }));
            out.state_error = Some(err);
        }
    }
    if opts.do_latency {
        let (latency, err) = latency(model, &opts.api_key, opts).await;
        out.latency = latency;
        if let Some(err) = err {
            if out.ping.as_ref().is_none_or(|p| p.ok) {
                out.state_error = Some(err);
            }
        }
    }
    out
}

#[derive(Debug, Default)]
struct SseParser {
    buffer: String,
    done: bool,
    tokens: u64,
    chunk_count: u64,
    accumulated_chars: usize,
    ttft_content: bool,
}

impl SseParser {
    fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
        while let Some(newline) = self.buffer.find('\n') {
            let line = self.buffer[..newline].trim_end_matches('\r').to_string();
            self.buffer = self.buffer[newline + 1..].to_string();
            self.consume_line(line.trim());
        }
    }

    fn consume_line(&mut self, line: &str) {
        if line.is_empty() || line.starts_with(':') {
            return;
        }
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            return;
        };
        if data == "[DONE]" {
            self.done = true;
            return;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };
        if let Some(tokens) = value
            .get("usage")
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(|v| v.as_u64())
        {
            self.tokens = self.tokens.max(tokens);
        }
        let Some(delta) = value
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
        else {
            return;
        };
        for field in ["content", "reasoning_content", "reasoning", "thinking"] {
            if let Some(text) = delta
                .get(field)
                .and_then(|v| v.as_str())
                .filter(|text| !text.is_empty())
            {
                self.ttft_content = true;
                self.chunk_count += 1;
                self.accumulated_chars += text.len();
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SseParser;

    #[test]
    fn parses_done_split_across_chunks() {
        let mut parser = SseParser::default();
        parser.push("data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n");
        parser.push("\ndata: [DO");
        parser.push("NE]\n");
        assert!(parser.done);
        assert!(parser.ttft_content);
    }

    #[test]
    fn parses_crlf_events_and_usage() {
        let mut parser = SseParser::default();
        parser.push("data: {\"usage\":{\"completion_tokens\":7}}\r\n\r\n");
        assert_eq!(parser.tokens, 7);
        assert!(!parser.done);
    }
}
