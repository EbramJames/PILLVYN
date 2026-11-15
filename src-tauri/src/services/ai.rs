use crate::config::AiConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPromptResponse {
    pub model: String,
    pub provider: String,
    pub reply: String,
}

pub async fn run_ai_prompt(
    cfg: &AiConfig,
    prompt: &str,
    api_key: Option<String>,
) -> Result<AiPromptResponse, String> {
    if !cfg.enabled {
        return Err(
            "Optional AI module is disabled. Enable it in Settings -> Integrations.".into(),
        );
    }
    let clean_prompt = prompt.trim();
    if clean_prompt.is_empty() {
        return Err("Prompt cannot be empty".into());
    }
    if clean_prompt.chars().count() > 2000 {
        return Err("Prompt is limited to 2000 characters".into());
    }

    let endpoint = cfg.endpoint.trim().trim_end_matches('/');
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err("AI endpoint must use http:// or https://".into());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    if cfg.provider == "ollama" {
        let url = format!("{endpoint}/api/generate");
        let body = serde_json::json!({
            "model": cfg.model,
            "prompt": clean_prompt,
            "stream": false
        });
        let resp = client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Local Ollama unreachable at {endpoint}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Ollama returned status {}", resp.status()));
        }
        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Invalid Ollama JSON response: {e}"))?;
        let reply = val["response"].as_str().unwrap_or("").trim().to_string();
        if reply.is_empty() {
            return Err("Ollama returned an empty response".into());
        }
        return Ok(AiPromptResponse {
            model: cfg.model.clone(),
            provider: "ollama".into(),
            reply,
        });
    }

    // OpenAI-compatible local or user-supplied endpoint (API key provided in-memory from user input/env, never saved in source)
    let url = format!("{endpoint}/v1/chat/completions");
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            { "role": "system", "content": "You are a concise assistant inside PILLVYN Dynamic Island." },
            { "role": "user", "content": clean_prompt }
        ]
    });
    let mut req = client.post(url).json(&body);
    let env_key = std::env::var("PILLVYN_AI_API_KEY").ok();
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()).or(env_key) {
        req = req.bearer_auth(key.trim());
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("AI endpoint unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("AI endpoint returned status {}", resp.status()));
    }
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid AI JSON response: {e}"))?;
    let reply = val["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if reply.is_empty() {
        return Err("AI endpoint returned an empty completion".into());
    }
    Ok(AiPromptResponse {
        model: cfg.model.clone(),
        provider: "openai_compatible".into(),
        reply,
    })
}
