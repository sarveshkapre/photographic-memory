use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub summary: String,
}

#[async_trait]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, image_path: &Path) -> Result<AnalysisResult>;
}

#[derive(Debug, Clone)]
pub struct MetadataAnalyzer;

#[async_trait]
impl Analyzer for MetadataAnalyzer {
    async fn analyze(&self, image_path: &Path) -> Result<AnalysisResult> {
        let metadata = std::fs::metadata(image_path)
            .with_context(|| format!("failed to read metadata for {}", image_path.display()))?;
        Ok(AnalysisResult {
            summary: format!(
                "Captured screenshot saved to {} ({} bytes).",
                image_path.display(),
                metadata.len()
            ),
        })
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiAnalyzer {
    client: Client,
    api_key: String,
    model: String,
    prompt: String,
    api_base_url: String,
    max_retries: u32,
    retry_base_delay: Duration,
}

impl OpenAiAnalyzer {
    const DEFAULT_API_BASE_URL: &'static str = "https://api.openai.com";
    const DEFAULT_MAX_RETRIES: u32 = 2;
    const DEFAULT_RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
    const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
    const MAX_ERROR_BODY_CHARS: usize = 500;

    pub fn new(api_key: String, model: String, prompt: String) -> Self {
        let client = build_client(Self::DEFAULT_REQUEST_TIMEOUT);
        Self {
            client,
            api_key,
            model,
            prompt,
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            max_retries: Self::DEFAULT_MAX_RETRIES,
            retry_base_delay: Self::DEFAULT_RETRY_BASE_DELAY,
        }
    }

    #[cfg(test)]
    fn new_for_test(
        api_key: String,
        model: String,
        prompt: String,
        api_base_url: String,
        timeout: Duration,
        max_retries: u32,
        retry_base_delay: Duration,
    ) -> Self {
        Self {
            client: build_client(timeout),
            api_key,
            model,
            prompt,
            api_base_url,
            max_retries,
            retry_base_delay,
        }
    }
}

#[async_trait]
impl Analyzer for OpenAiAnalyzer {
    async fn analyze(&self, image_path: &Path) -> Result<AnalysisResult> {
        let image_bytes = std::fs::read(image_path)
            .with_context(|| format!("failed to read screenshot {}", image_path.display()))?;
        let base64_image = general_purpose::STANDARD.encode(image_bytes);
        let data_url = format!("data:image/png;base64,{base64_image}");

        let body = json!({
            "model": self.model,
            "input": [
                {
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": self.prompt},
                        {"type": "input_image", "image_url": data_url}
                    ]
                }
            ]
        });

        let endpoint = format!("{}/v1/responses", self.api_base_url.trim_end_matches('/'));
        let mut attempt = 0u32;

        loop {
            let response_result = self
                .client
                .post(&endpoint)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await;

            match response_result {
                Ok(response) => {
                    let status = response.status();
                    let response_body = response.text().await.unwrap_or_default();
                    if status.is_success() {
                        let summary = summary_from_response_body(&response_body);
                        return Ok(AnalysisResult { summary });
                    }

                    if should_retry_status(status) && attempt < self.max_retries {
                        let delay = retry_delay(self.retry_base_delay, attempt);
                        attempt += 1;
                        sleep(delay).await;
                        continue;
                    }

                    bail!(
                        "OpenAI API error {status}: {}",
                        truncate_error_body(&response_body, Self::MAX_ERROR_BODY_CHARS)
                    );
                }
                Err(error) => {
                    if should_retry_error(&error) && attempt < self.max_retries {
                        let delay = retry_delay(self.retry_base_delay, attempt);
                        attempt += 1;
                        sleep(delay).await;
                        continue;
                    }

                    return Err(error).context("failed to call OpenAI Responses API");
                }
            }
        }
    }
}

fn extract_text(root: &Value) -> Option<String> {
    if let Some(value) = root.get("output_text")
        && let Some(text) = value.as_str()
    {
        return Some(text.to_string());
    }

    let output = root.get("output")?.as_array()?;
    let mut fragments = Vec::new();

    for item in output {
        let content = item.get("content").and_then(Value::as_array);
        if let Some(content_parts) = content {
            for part in content_parts {
                let part_type = part.get("type").and_then(Value::as_str).unwrap_or_default();
                if matches!(part_type, "output_text" | "text")
                    && let Some(text) = part.get("text").and_then(Value::as_str)
                {
                    fragments.push(text.trim().to_string());
                }
            }
        }
    }

    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

fn build_client(timeout: Duration) -> Client {
    match Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(_) => Client::new(),
    }
}

fn should_retry_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::CONFLICT
        || status.is_server_error()
}

fn retry_delay(base: Duration, attempt: u32) -> Duration {
    let factor = 1u32.checked_shl(attempt.min(6)).unwrap_or(64);
    base.checked_mul(factor)
        .unwrap_or_else(|| Duration::from_secs(30))
}

fn truncate_error_body(body: &str, limit: usize) -> String {
    let mut snippet = body.trim().to_string();
    if snippet.len() > limit {
        snippet.truncate(limit);
        snippet.push_str("...");
    }
    if snippet.is_empty() {
        "<empty response body>".to_string()
    } else {
        snippet
    }
}

fn summary_from_response_body(response_body: &str) -> String {
    match serde_json::from_str::<Value>(response_body) {
        Ok(json) => extract_text(&json)
            .or_else(|| {
                json.pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| {
                "No textual output returned by model; response stored as metadata only.".to_string()
            }),
        Err(_) => format!(
            "Model returned non-JSON response: {}",
            truncate_error_body(response_body, OpenAiAnalyzer::MAX_ERROR_BODY_CHARS)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{Analyzer, OpenAiAnalyzer, extract_text};
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    #[test]
    fn extracts_output_text_field_when_present() {
        let value = json!({"output_text": "summary"});
        assert_eq!(extract_text(&value), Some("summary".to_string()));
    }

    #[test]
    fn extracts_text_from_output_content() {
        let value = json!({
            "output": [
                {
                    "content": [
                        {"type": "output_text", "text": "line 1"},
                        {"type": "text", "text": "line 2"}
                    ]
                }
            ]
        });
        assert_eq!(extract_text(&value), Some("line 1\nline 2".to_string()));
    }

    #[tokio::test]
    async fn retries_transient_http_error_and_succeeds() {
        let responses = vec![
            MockHttpResponse::new(
                429,
                r#"{"error":{"message":"rate limit hit"}}"#,
                Duration::ZERO,
            ),
            MockHttpResponse::new(200, r#"{"output_text":"summary ok"}"#, Duration::ZERO),
        ];
        let (base_url, hit_count, server) = spawn_mock_server(responses).await;
        let (_temp_dir, image_path) = write_test_image();
        let analyzer = OpenAiAnalyzer::new_for_test(
            "test-key".to_string(),
            "gpt-5".to_string(),
            "prompt".to_string(),
            base_url,
            Duration::from_secs(2),
            2,
            Duration::from_millis(1),
        );

        let result = analyzer
            .analyze(&image_path)
            .await
            .expect("retry should recover");
        assert_eq!(result.summary, "summary ok");
        assert_eq!(hit_count.load(Ordering::SeqCst), 2);
        server.await.expect("mock server should finish");
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable_http_error() {
        let responses = vec![MockHttpResponse::new(
            400,
            r#"{"error":{"message":"bad request"}}"#,
            Duration::ZERO,
        )];
        let (base_url, hit_count, server) = spawn_mock_server(responses).await;
        let (_temp_dir, image_path) = write_test_image();
        let analyzer = OpenAiAnalyzer::new_for_test(
            "test-key".to_string(),
            "gpt-5".to_string(),
            "prompt".to_string(),
            base_url,
            Duration::from_secs(2),
            3,
            Duration::from_millis(1),
        );

        let err = analyzer
            .analyze(&image_path)
            .await
            .expect_err("non-retryable error should fail immediately");
        assert!(err.to_string().contains("OpenAI API error 400"));
        assert_eq!(hit_count.load(Ordering::SeqCst), 1);
        server.await.expect("mock server should finish");
    }

    #[tokio::test]
    async fn times_out_when_response_exceeds_client_timeout() {
        let responses = vec![MockHttpResponse::new(
            200,
            r#"{"output_text":"delayed"}"#,
            Duration::from_millis(200),
        )];
        let (base_url, _hit_count, server) = spawn_mock_server(responses).await;
        let (_temp_dir, image_path) = write_test_image();
        let analyzer = OpenAiAnalyzer::new_for_test(
            "test-key".to_string(),
            "gpt-5".to_string(),
            "prompt".to_string(),
            base_url,
            Duration::from_millis(30),
            0,
            Duration::from_millis(1),
        );

        let err = analyzer
            .analyze(&image_path)
            .await
            .expect_err("timeout should fail when retries disabled");
        assert!(
            err.to_string()
                .contains("failed to call OpenAI Responses API")
        );
        server.await.expect("mock server should finish");
    }

    #[tokio::test]
    async fn falls_back_when_success_payload_is_not_json() {
        let responses = vec![MockHttpResponse::new(200, "not-json", Duration::ZERO)];
        let (base_url, _hit_count, server) = spawn_mock_server(responses).await;
        let (_temp_dir, image_path) = write_test_image();
        let analyzer = OpenAiAnalyzer::new_for_test(
            "test-key".to_string(),
            "gpt-5".to_string(),
            "prompt".to_string(),
            base_url,
            Duration::from_secs(2),
            0,
            Duration::from_millis(1),
        );

        let result = analyzer
            .analyze(&image_path)
            .await
            .expect("malformed success payload should be summarized");
        assert!(result.summary.contains("Model returned non-JSON response"));
        server.await.expect("mock server should finish");
    }

    #[derive(Debug, Clone)]
    struct MockHttpResponse {
        status: u16,
        body: String,
        delay: Duration,
    }

    impl MockHttpResponse {
        fn new(status: u16, body: impl Into<String>, delay: Duration) -> Self {
            Self {
                status,
                body: body.into(),
                delay,
            }
        }
    }

    async fn spawn_mock_server(
        responses: Vec<MockHttpResponse>,
    ) -> (String, Arc<AtomicUsize>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("listener addr");
        let hit_count = Arc::new(AtomicUsize::new(0));
        let hit_count_for_task = Arc::clone(&hit_count);
        let handle = tokio::spawn(async move {
            for response in responses {
                let (mut stream, _) = listener.accept().await.expect("accept should succeed");
                let local_hit_count = Arc::clone(&hit_count_for_task);
                tokio::spawn(async move {
                    local_hit_count.fetch_add(1, Ordering::SeqCst);
                    let mut read_buf = [0u8; 1024];
                    let _ = stream.read(&mut read_buf).await;
                    if !response.delay.is_zero() {
                        tokio::time::sleep(response.delay).await;
                    }
                    let payload = response.body.as_bytes();
                    let reason = match response.status {
                        200 => "OK",
                        400 => "Bad Request",
                        408 => "Request Timeout",
                        409 => "Conflict",
                        429 => "Too Many Requests",
                        500 => "Internal Server Error",
                        _ => "Unknown",
                    };
                    let raw_response = format!(
                        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response.status,
                        reason,
                        payload.len(),
                        response.body
                    );
                    let _ = stream.write_all(raw_response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        (format!("http://{addr}"), hit_count, handle)
    }

    fn write_test_image() -> (tempfile::TempDir, std::path::PathBuf) {
        let temp_dir = tempdir().expect("tempdir");
        let image_path = temp_dir.path().join("capture.png");
        std::fs::write(&image_path, b"fake image bytes").expect("test image");
        (temp_dir, image_path)
    }
}
