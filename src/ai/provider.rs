use std::sync::Mutex;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub provider_name: String,
    pub model_name: String,
}

pub trait AiProvider: Send + Sync {
    fn health(&self) -> Result<bool>;
    fn list_models(&self) -> Result<Vec<String>>;
    fn generate_json(&self, system: &str, user: &str) -> Result<String>;
    fn info(&self) -> ModelInfo;
}

pub struct FakeProvider {
    pub response: Mutex<std::result::Result<String, String>>,
    pub models: Vec<String>,
    pub info: ModelInfo,
}

impl FakeProvider {
    pub fn success(response_json: impl Into<String>) -> Self {
        Self {
            response: Mutex::new(Ok(response_json.into())),
            models: vec!["fake-coder:7b".to_string()],
            info: ModelInfo {
                provider_name: "fake".to_string(),
                model_name: "fake-coder:7b".to_string(),
            },
        }
    }

    pub fn error(error_message: impl Into<String>) -> Self {
        Self {
            response: Mutex::new(Err(error_message.into())),
            models: vec![],
            info: ModelInfo {
                provider_name: "fake".to_string(),
                model_name: "none".to_string(),
            },
        }
    }
}

impl AiProvider for FakeProvider {
    fn health(&self) -> Result<bool> {
        let lock = self.response.lock().unwrap();
        Ok(lock.is_ok())
    }

    fn list_models(&self) -> Result<Vec<String>> {
        Ok(self.models.clone())
    }

    fn generate_json(&self, _system: &str, _user: &str) -> Result<String> {
        let lock = self.response.lock().unwrap();
        match &*lock {
            Ok(s) => Ok(s.clone()),
            Err(err) => Err(AppError::Ai(err.clone())),
        }
    }

    fn info(&self) -> ModelInfo {
        self.info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_provider_success() {
        let provider = FakeProvider::success(r#"{"items":[]}"#);
        assert!(provider.health().unwrap());
        assert_eq!(provider.list_models().unwrap(), vec!["fake-coder:7b"]);
        assert_eq!(
            provider.generate_json("sys", "user").unwrap(),
            r#"{"items":[]}"#
        );
    }

    #[test]
    fn fake_provider_error() {
        let provider = FakeProvider::error("Service Unavailable");
        assert!(!provider.health().unwrap());
        let res = provider.generate_json("sys", "user");
        assert!(matches!(res, Err(AppError::Ai(_))));
    }
}
