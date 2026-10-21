use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Origin {
    #[default]
    Deterministic,
    Ai {
        provider: String,
        model: String,
    },
    External {
        source: String,
    },
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deterministic => write!(f, "Deterministic"),
            Self::Ai { provider, model } => write!(f, "AI ({provider}/{model})"),
            Self::External { source } => write!(f, "External ({source})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_json_roundtrip() {
        let origins = vec![
            Origin::Deterministic,
            Origin::Ai {
                provider: "ollama".to_string(),
                model: "qwen2.5-coder".to_string(),
            },
            Origin::External {
                source: "github-pr".to_string(),
            },
        ];

        for origin in origins {
            let json = serde_json::to_string(&origin).expect("serialize origin");
            let deserialized: Origin = serde_json::from_str(&json).expect("deserialize origin");
            assert_eq!(origin, deserialized);
        }
    }
}
