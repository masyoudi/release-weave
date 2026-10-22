use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use super::provider::{AiProvider, ModelInfo};
use crate::errors::{AppError, Result};

pub struct OllamaProvider {
    pub base_url: String,
    pub model: String,
    pub timeout: Duration,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaModelItem>>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelItem {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaChatMessage>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatMessage {
    content: Option<String>,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            timeout: Duration::from_secs(60),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn agent(&self, timeout: Duration) -> ureq::Agent {
        ureq::config::Config::builder()
            .timeout_global(Some(timeout))
            .build()
            .new_agent()
    }
}

impl AiProvider for OllamaProvider {
    fn health(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self.agent(Duration::from_secs(3)).get(&url).call();

        match resp {
            Ok(r) => Ok(r.status() == 200),
            Err(_) => Ok(false),
        }
    }

    fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .agent(self.timeout)
            .get(&url)
            .call()
            .map_err(|e| {
                AppError::Ai(format!(
                    "Failed to connect to Ollama at {}. Ensure Ollama is running (`ollama serve`). Details: {e}",
                    self.base_url
                ))
            })?;

        let tags: OllamaTagsResponse = resp
            .into_body()
            .read_json()
            .map_err(|e| AppError::Ai(format!("Failed to parse Ollama /api/tags response: {e}")))?;

        let list = tags
            .models
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.name)
            .collect();

        Ok(list)
    }

    fn generate_json(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let payload = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "stream": false,
            "format": "json"
        });

        let resp = self
            .agent(self.timeout)
            .post(&url)
            .send_json(payload)
            .map_err(|e| {
                AppError::Ai(format!(
                    "Failed to communicate with Ollama at {}: {e}. Check if model '{}' is pulled (`ollama pull {}`)",
                    self.base_url, self.model, self.model
                ))
            })?;

        let chat_resp: OllamaChatResponse = resp.into_body().read_json().map_err(|e| {
            AppError::Ai(format!("Invalid JSON response structure from Ollama: {e}"))
        })?;

        if let Some(err) = chat_resp.error {
            return Err(AppError::Ai(format!("Ollama error: {err}")));
        }

        let content = chat_resp
            .message
            .and_then(|m| m.content)
            .ok_or_else(|| AppError::Ai("Ollama returned empty response message".to_string()))?;

        Ok(content)
    }

    fn info(&self) -> ModelInfo {
        ModelInfo {
            provider_name: "ollama".to_string(),
            model_name: self.model.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn send_mock_response(stream: &mut std::net::TcpStream, status: &str, body: &str) {
        let resp = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));
        let mut drain = [0; 512];
        let _ = stream.read(&mut drain);
    }

    #[test]
    fn ollama_success_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = stream.unwrap();
                let mut buffer = [0; 2048];
                let _ = stream.read(&mut buffer);

                let body = r#"{"message":{"content":"{\"items\":[]}"},"models":[{"name":"qwen2.5-coder:7b"}]}"#;
                send_mock_response(&mut stream, "200 OK", body);
            }
        });

        let provider = OllamaProvider::new(format!("http://127.0.0.1:{port}"), "qwen2.5-coder:7b");

        let models = provider.list_models().unwrap();
        assert_eq!(models, vec!["qwen2.5-coder:7b"]);

        let output = provider.generate_json("sys", "user").unwrap();
        assert_eq!(output, r#"{"items":[]}"#);
    }

    #[test]
    fn ollama_handles_500_server_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buffer = [0; 2048];
                let _ = stream.read(&mut buffer);

                send_mock_response(
                    &mut stream,
                    "500 Internal Server Error",
                    r#"{"error":"model not found"}"#,
                );
            }
        });

        let provider =
            OllamaProvider::new(format!("http://127.0.0.1:{port}"), "non-existent-model");

        let res = provider.generate_json("sys", "user");
        assert!(matches!(res, Err(AppError::Ai(_))));
    }
}
