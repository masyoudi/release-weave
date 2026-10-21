use crate::errors::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteProvider {
    GitHub,
    GitLab,
    Custom(String),
}

impl RemoteProvider {
    pub fn display_name(&self) -> &str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
            Self::Custom(name) => name.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    pub host: String,
    pub owner_path: String,
    pub repo: String,
    pub provider: RemoteProvider,
}

pub struct RemoteService;

impl RemoteService {
    pub fn parse_remote_url(url: &str) -> Result<RemoteInfo> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "Remote URL cannot be empty".to_string(),
            ));
        }

        let (host, path) = if let Some(ssh_rem) = trimmed.strip_prefix("git@") {
            if let Some((h, p)) = ssh_rem.split_once(':') {
                (h, p)
            } else if let Some((h, p)) = ssh_rem.split_once('/') {
                (h, p)
            } else {
                return Err(AppError::Validation(format!(
                    "Invalid SSH remote URL: {url}"
                )));
            }
        } else if let Some(stripped) = trimmed.strip_prefix("ssh://") {
            let after_user = if let Some((_, rest)) = stripped.split_once('@') {
                rest
            } else {
                stripped
            };
            if let Some((h_port, p)) = after_user.split_once('/') {
                let h = if let Some((h, _)) = h_port.split_once(':') {
                    h
                } else {
                    h_port
                };
                (h, p)
            } else {
                return Err(AppError::Validation(format!("Invalid ssh:// URL: {url}")));
            }
        } else if let Some(stripped) = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
        {
            let after_user = if let Some((_, rest)) = stripped.split_once('@') {
                rest
            } else {
                stripped
            };
            if let Some((h_port, p)) = after_user.split_once('/') {
                let h = if let Some((h, _)) = h_port.split_once(':') {
                    h
                } else {
                    h_port
                };
                (h, p)
            } else {
                return Err(AppError::Validation(format!(
                    "Invalid HTTP/HTTPS remote URL: {url}"
                )));
            }
        } else {
            return Err(AppError::Validation(format!(
                "Unrecognized remote URL format: {url}"
            )));
        };

        let host = host.trim().to_lowercase();
        let clean_path = path.trim_start_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = clean_path.split('/').filter(|p| !p.is_empty()).collect();

        if parts.len() < 2 {
            return Err(AppError::Validation(format!(
                "Remote URL path must contain at least owner and repo: '{clean_path}'"
            )));
        }

        for part in &parts {
            if *part == "." || *part == ".." || part.contains('\\') {
                return Err(AppError::Validation(format!(
                    "Remote URL path contains invalid traversal component: '{part}'"
                )));
            }
            if !part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                return Err(AppError::Validation(format!(
                    "Remote URL path component '{part}' contains invalid characters"
                )));
            }
        }

        let repo = parts.last().unwrap().to_string();
        let owner_path = parts[..parts.len() - 1].join("/");

        let provider = if host == "github.com" || host.contains("github.") {
            RemoteProvider::GitHub
        } else if host == "gitlab.com" || host.contains("gitlab.") {
            RemoteProvider::GitLab
        } else {
            RemoteProvider::Custom(host.clone())
        };

        Ok(RemoteInfo {
            host,
            owner_path,
            repo,
            provider,
        })
    }

    pub fn detect_remote(repo: &git2::Repository, remote_name: Option<&str>) -> Result<RemoteInfo> {
        let name = remote_name.unwrap_or("origin");
        let remote = repo
            .find_remote(name)
            .map_err(|e| AppError::NotFound(format!("Git remote '{name}' not found: {e}")))?;
        let url = remote.url().map_err(|e| {
            AppError::Validation(format!("Git remote '{name}' has no valid URL: {e}"))
        })?;
        Self::parse_remote_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_ssh_and_https() {
        let ssh = RemoteService::parse_remote_url("git@github.com:release-weave/core.git").unwrap();
        assert_eq!(ssh.host, "github.com");
        assert_eq!(ssh.owner_path, "release-weave");
        assert_eq!(ssh.repo, "core");
        assert_eq!(ssh.provider, RemoteProvider::GitHub);

        let https =
            RemoteService::parse_remote_url("https://github.com/release-weave/core").unwrap();
        assert_eq!(https.host, "github.com");
        assert_eq!(https.owner_path, "release-weave");
        assert_eq!(https.repo, "core");
        assert_eq!(https.provider, RemoteProvider::GitHub);
    }

    #[test]
    fn parses_gitlab_subgroup_and_self_hosted() {
        let sub =
            RemoteService::parse_remote_url("git@gitlab.com:org/subgroup/nested/microservice.git")
                .unwrap();
        assert_eq!(sub.host, "gitlab.com");
        assert_eq!(sub.owner_path, "org/subgroup/nested");
        assert_eq!(sub.repo, "microservice");
        assert_eq!(sub.provider, RemoteProvider::GitLab);

        let self_hosted = RemoteService::parse_remote_url(
            "https://gitlab.internal.enterprise.com/backend/payments.git",
        )
        .unwrap();
        assert_eq!(self_hosted.host, "gitlab.internal.enterprise.com");
        assert_eq!(self_hosted.owner_path, "backend");
        assert_eq!(self_hosted.repo, "payments");
        assert_eq!(self_hosted.provider, RemoteProvider::GitLab);
    }

    #[test]
    fn parses_custom_host() {
        let custom =
            RemoteService::parse_remote_url("https://git.server.local/devops/deployer.git")
                .unwrap();
        assert_eq!(custom.host, "git.server.local");
        assert_eq!(custom.owner_path, "devops");
        assert_eq!(custom.repo, "deployer");
        assert_eq!(
            custom.provider,
            RemoteProvider::Custom("git.server.local".to_string())
        );
    }

    #[test]
    fn rejects_malformed_and_unknown_urls() {
        assert!(RemoteService::parse_remote_url("").is_err());
        assert!(RemoteService::parse_remote_url("ftp://server/repo").is_err());
        assert!(RemoteService::parse_remote_url("https://github.com/onlyone").is_err());
    }

    #[test]
    fn rejects_path_traversal_in_remote_urls() {
        assert!(
            RemoteService::parse_remote_url("https://github.com/attacker/../../api/repo.git")
                .is_err()
        );
        assert!(RemoteService::parse_remote_url("git@github.com:evil/../admin/pwned.git").is_err());
        assert!(RemoteService::parse_remote_url("https://github.com/org/sub/./repo.git").is_err());
    }
}
