use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub id: Option<i64>,
    pub project_id: i64,
    pub name: String,
    pub relative_path: String,
    pub manifest_path: String,
    pub current_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Package {
    pub fn new(
        project_id: i64,
        name: impl Into<String>,
        relative_path: impl Into<String>,
        manifest_path: impl Into<String>,
        current_version: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            project_id,
            name: name.into(),
            relative_path: relative_path.into(),
            manifest_path: manifest_path.into(),
            current_version,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_serde_roundtrip() {
        let pkg = Package::new(
            1,
            "my-lib",
            "crates/my-lib",
            "crates/my-lib/Cargo.toml",
            Some("0.1.0".to_string()),
        );
        let json = serde_json::to_string(&pkg).unwrap();
        let de: Package = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg.name, de.name);
        assert_eq!(pkg.relative_path, de.relative_path);
        assert_eq!(pkg.current_version, de.current_version);
    }
}
