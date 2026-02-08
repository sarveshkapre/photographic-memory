use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde_json::{Value, json};
use std::path::Path;

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
}

impl OpenAiAnalyzer {
    pub fn new(api_key: String, model: String, prompt: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            prompt,
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

        let response = self
            .client
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("failed to call OpenAI Responses API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("OpenAI API error {status}: {body}");
        }

        let json: Value = response
            .json()
            .await
            .context("failed to decode OpenAI response JSON")?;

        let summary = extract_text(&json)
            .or_else(|| {
                json.pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| {
                "No textual output returned by model; response stored as metadata only.".to_string()
            });

        Ok(AnalysisResult { summary })
    }
}

fn extract_text(root: &Value) -> Option<String> {
    if let Some(value) = root.get("output_text") {
        if let Some(text) = value.as_str() {
            return Some(text.to_string());
        }
    }

    let output = root.get("output")?.as_array()?;
    let mut fragments = Vec::new();

    for item in output {
        let content = item.get("content").and_then(Value::as_array);
        if let Some(content_parts) = content {
            for part in content_parts {
                let part_type = part.get("type").and_then(Value::as_str).unwrap_or_default();
                if matches!(part_type, "output_text" | "text") {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        fragments.push(text.trim().to_string());
                    }
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

#[cfg(test)]
mod tests {
    use super::extract_text;
    use serde_json::json;

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
}
