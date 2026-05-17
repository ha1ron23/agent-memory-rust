use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct OllamaResponse {
    embedding: Vec<f32>,
}

pub async fn fetch_embedding(text: &str, client: &Client) -> Result<Vec<f32>, String> {
    let url = "http://localhost:11434/api/embeddings";
    let req = OllamaRequest {
        model: "nomic-embed-text".to_string(),
        prompt: text.to_string(),
    };
    let resp = client
        .post(url)
        .json(&req)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Ollama error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let data: OllamaResponse = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;
    Ok(data.embedding)
}