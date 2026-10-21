use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::errors::{AppError, Result};

pub const SERVICE_NAME: &str = "com.release_weave.tokens";

pub trait TokenStore: Send + Sync {
    fn store_token(&self, host: &str, token: &str) -> Result<()>;
    fn get_token(&self, host: &str) -> Result<Option<String>>;
    fn delete_token(&self, host: &str) -> Result<()>;
}

/// OS-backed keyring implementation using `keyring` crate.
pub struct KeyringTokenStore;

impl TokenStore for KeyringTokenStore {
    fn store_token(&self, host: &str, token: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, host)
            .map_err(|e| AppError::Validation(format!("Keyring init error: {e}")))?;
        entry
            .set_password(token)
            .map_err(|e| AppError::Validation(format!("Keyring store error: {e}")))?;
        Ok(())
    }

    fn get_token(&self, host: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, host)
            .map_err(|e| AppError::Validation(format!("Keyring init error: {e}")))?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Validation(format!("Keyring get error: {e}"))),
        }
    }

    fn delete_token(&self, host: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, host)
            .map_err(|e| AppError::Validation(format!("Keyring init error: {e}")))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Validation(format!("Keyring delete error: {e}"))),
        }
    }
}

/// In-memory mock token store for deterministic testing without OS keyring daemon.
#[derive(Default, Clone)]
pub struct MockTokenStore {
    tokens: Arc<Mutex<HashMap<String, String>>>,
}

impl MockTokenStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenStore for MockTokenStore {
    fn store_token(&self, host: &str, token: &str) -> Result<()> {
        let mut map = self
            .tokens
            .lock()
            .map_err(|e| AppError::Validation(format!("Mock store lock error: {e}")))?;
        map.insert(host.to_string(), token.to_string());
        Ok(())
    }

    fn get_token(&self, host: &str) -> Result<Option<String>> {
        let map = self
            .tokens
            .lock()
            .map_err(|e| AppError::Validation(format!("Mock store lock error: {e}")))?;
        Ok(map.get(host).cloned())
    }

    fn delete_token(&self, host: &str) -> Result<()> {
        let mut map = self
            .tokens
            .lock()
            .map_err(|e| AppError::Validation(format!("Mock store lock error: {e}")))?;
        map.remove(host);
        Ok(())
    }
}

static CUSTOM_STORE: OnceLock<Arc<dyn TokenStore>> = OnceLock::new();

pub struct SecretService;

impl SecretService {
    pub fn set_custom_store(store: Arc<dyn TokenStore>) {
        let _ = CUSTOM_STORE.set(store);
    }

    fn current_store() -> &'static dyn TokenStore {
        if let Some(store) = CUSTOM_STORE.get() {
            store.as_ref()
        } else {
            &KeyringTokenStore
        }
    }

    pub fn store_token(host: &str, token: &str) -> Result<()> {
        Self::current_store().store_token(host, token)
    }

    pub fn get_token(host: &str) -> Result<Option<String>> {
        Self::current_store().get_token(host)
    }

    pub fn delete_token(host: &str) -> Result<()> {
        Self::current_store().delete_token(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::redact::redact;

    #[test]
    fn mock_store_crud_lifecycle() {
        let store = MockTokenStore::new();

        assert_eq!(store.get_token("github.com").unwrap(), None);

        store
            .store_token("github.com", "ghp_1234567890abcdef1234567890")
            .unwrap();
        assert_eq!(
            store.get_token("github.com").unwrap().as_deref(),
            Some("ghp_1234567890abcdef1234567890")
        );

        store
            .store_token("gitlab.com", "glpat-secrettoken1234567890")
            .unwrap();
        assert_eq!(
            store.get_token("gitlab.com").unwrap().as_deref(),
            Some("glpat-secrettoken1234567890")
        );

        store.delete_token("github.com").unwrap();
        assert_eq!(store.get_token("github.com").unwrap(), None);
        assert_eq!(
            store.get_token("gitlab.com").unwrap().as_deref(),
            Some("glpat-secrettoken1234567890")
        );
    }

    #[test]
    fn tokens_are_redacted_in_logs() {
        let gh_token = "ghp_9876543210zyxwvutsrqponmlkjihg";
        let gl_token = "glpat-abcdef1234567890qwertyuiop";

        let log_line = format!("Connecting to host with {gh_token} and {gl_token}");
        let sanitized = redact(&log_line);

        assert!(!sanitized.contains(gh_token));
        assert!(!sanitized.contains(gl_token));
        assert!(sanitized.contains("ghp_***"));
        assert!(sanitized.contains("glpat-***"));
    }
}
