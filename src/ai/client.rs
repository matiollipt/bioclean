use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct OllamaClient {
    pub base_url: String,
    client: reqwest::blocking::Client,
}

#[derive(Debug, Deserialize)]
struct TagListResponse {
    models: Option<Vec<ModelTag>>,
}

#[derive(Debug, Deserialize)]
struct ModelTag {
    name: String,
}

#[derive(Debug, Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    pub fn is_online(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client.get(&url).send().map(|r| r.status().is_success()).unwrap_or(false)
    }

    pub fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self.client.get(&url).send().context("Failed to connect to Ollama API")?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama returned HTTP error: {}", resp.status());
        }
        let tag_list: TagListResponse = resp.json().context("Failed to parse Ollama tags response")?;
        let names = tag_list.models.unwrap_or_default().into_iter().map(|m| m.name).collect();
        Ok(names)
    }

    pub fn select_best_model(&self, requested: Option<&str>) -> String {
        let available = self.list_models().unwrap_or_default();
        if let Some(req) = requested {
            if !req.is_empty() {
                if available.iter().any(|m| m == req || m.starts_with(req)) {
                    return req.to_string();
                }
            }
        }

        // Priority candidates
        let candidates = [
            "qwen2.5-coder:7b",
            "qwen3.5:4b",
            "gemma4:e2b",
            "smallthinker:latest",
            "qwen2.5:0.5b",
            "llama3:latest",
            "mistral:latest",
        ];

        for cand in candidates {
            if let Some(found) = available.iter().find(|m| m.as_str() == cand || m.starts_with(cand)) {
                return found.clone();
            }
        }

        if let Some(first) = available.first() {
            return first.clone();
        }

        "qwen2.5-coder:7b".to_string()
    }

    pub fn generate(&self, model: &str, prompt: &str, system: Option<&str>) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        let body = GenerateRequest {
            model,
            prompt,
            system,
            stream: false,
        };

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .context("Failed to call Ollama generate API")?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama generate returned error status: {}", resp.status());
        }

        let gen_resp: GenerateResponse = resp.json().context("Failed to parse Ollama generate response")?;
        Ok(gen_resp.response.trim().to_string())
    }
}
