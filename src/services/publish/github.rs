use std::time::Duration;

use serde_json::json;

use crate::errors::{AppError, Result};
use crate::services::publish::PublishedRelease;

pub struct GitHubClient {
    base_url: String,
    token: String,
    timeout: Duration,
}

impl GitHubClient {
    pub fn new(host: &str, token: &str) -> Self {
        let base_url = if host == "github.com" {
            "https://api.github.com".to_string()
        } else if host.starts_with("http://") || host.starts_with("https://") {
            host.trim_end_matches('/').to_string()
        } else {
            format!("https://{host}/api/v3")
        };

        Self {
            base_url,
            token: token.trim().to_string(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    fn agent(&self) -> ureq::Agent {
        ureq::config::Config::builder()
            .timeout_global(Some(self.timeout))
            .http_status_as_error(false)
            .build()
            .new_agent()
    }

    pub fn create_or_update_release(
        &self,
        owner: &str,
        repo: &str,
        tag: &str,
        name: &str,
        body: &str,
        draft: bool,
    ) -> Result<PublishedRelease> {
        let owner = owner.trim();
        let repo = repo.trim();
        let tag = tag.trim();

        if owner.is_empty() || repo.is_empty() || tag.is_empty() {
            return Err(AppError::Validation(
                "Owner, repository, and tag cannot be empty".to_string(),
            ));
        }
        if owner.contains("..") || repo.contains("..") || tag.contains("..") {
            return Err(AppError::Validation(
                "Path traversal sequences (..) are not allowed in release parameters".to_string(),
            ));
        }

        let check_url = format!("{}/repos/{owner}/{repo}/releases/tags/{tag}", self.base_url);
        let agent = self.agent();

        let resp = agent
            .get(&check_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "ReleaseWeave")
            .call()
            .map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    format!("GitHub connection error: {e}"),
                ))
            })?;

        let status = resp.status().as_u16();
        if status == 200 {
            // Already exists -> PATCH /repos/{owner}/{repo}/releases/{id}
            let val: serde_json::Value = resp.into_body().read_json().map_err(|e| {
                AppError::Validation(format!("Failed to parse release response: {e}"))
            })?;

            let id = val["id"].as_i64().ok_or_else(|| {
                AppError::Validation("Missing 'id' field in GitHub release".to_string())
            })?;
            let html_url = val["html_url"].as_str().unwrap_or_default().to_string();

            let update_url = format!("{}/repos/{owner}/{repo}/releases/{id}", self.base_url);
            let patch_resp = agent
                .patch(&update_url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github+json")
                .header("User-Agent", "ReleaseWeave")
                .send_json(json!({
                    "name": name,
                    "body": body,
                    "draft": draft,
                }))
                .map_err(|e| {
                    AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("GitHub update error: {e}"),
                    ))
                })?;

            let patch_status = patch_resp.status().as_u16();
            if patch_status == 200 {
                let updated_val: serde_json::Value =
                    patch_resp.into_body().read_json().unwrap_or_default();
                let final_url = updated_val["html_url"]
                    .as_str()
                    .unwrap_or(&html_url)
                    .to_string();
                Ok(PublishedRelease {
                    provider: "GitHub".to_string(),
                    host: self.base_url.clone(),
                    release_url: final_url,
                    release_id: id.to_string(),
                    tag_name: tag.to_string(),
                    is_draft: draft,
                })
            } else {
                let msg = patch_resp.into_body().read_to_string().unwrap_or_default();
                Err(Self::handle_error(patch_status, &msg))
            }
        } else if status == 404 {
            // Does not exist -> POST /repos/{owner}/{repo}/releases
            let create_url = format!("{}/repos/{owner}/{repo}/releases", self.base_url);
            let post_resp = agent
                .post(&create_url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github+json")
                .header("User-Agent", "ReleaseWeave")
                .send_json(json!({
                    "tag_name": tag,
                    "name": name,
                    "body": body,
                    "draft": draft,
                }))
                .map_err(|e| {
                    AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("GitHub create error: {e}"),
                    ))
                })?;

            let post_status = post_resp.status().as_u16();
            if post_status == 201 {
                let created_val: serde_json::Value =
                    post_resp.into_body().read_json().map_err(|e| {
                        AppError::Validation(format!(
                            "Failed to parse created release response: {e}"
                        ))
                    })?;
                let id = created_val["id"].as_i64().unwrap_or_default().to_string();
                let html_url = created_val["html_url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                Ok(PublishedRelease {
                    provider: "GitHub".to_string(),
                    host: self.base_url.clone(),
                    release_url: html_url,
                    release_id: id,
                    tag_name: tag.to_string(),
                    is_draft: draft,
                })
            } else {
                let msg = post_resp.into_body().read_to_string().unwrap_or_default();
                Err(Self::handle_error(post_status, &msg))
            }
        } else {
            let msg = resp.into_body().read_to_string().unwrap_or_default();
            Err(Self::handle_error(status, &msg))
        }
    }

    fn handle_error(status: u16, msg: &str) -> AppError {
        match status {
            401 => AppError::Validation(
                "GitHub authentication failed (401 Unauthorized): Token is invalid or expired. Please update token in Settings.".to_string(),
            ),
            403 => AppError::Validation(format!(
                "GitHub request forbidden or rate limit exceeded (403): {msg}"
            )),
            404 => AppError::Validation(format!(
                "GitHub repository or release not found (404): {msg}"
            )),
            422 => AppError::Validation(format!(
                "GitHub validation failed (422 Unprocessable Entity): {msg}"
            )),
            _ => AppError::Validation(format!("GitHub API error (HTTP {status}): {msg}")),
        }
    }

    pub fn test_connection(&self) -> Result<String> {
        let url = format!("{}/user", self.base_url);
        let resp = self
            .agent()
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "ReleaseWeave")
            .call()
            .map_err(|e| AppError::Validation(format!("GitHub connection error: {e}")))?;

        let status = resp.status().as_u16();
        if status == 200 {
            let val: serde_json::Value = resp.into_body().read_json().unwrap_or_default();
            let login = val["login"].as_str().unwrap_or("authenticated");
            Ok(format!("Connected as GitHub user: @{login}"))
        } else {
            let msg = resp.into_body().read_to_string().unwrap_or_default();
            Err(Self::handle_error(status, &msg))
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
    fn github_release_create_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            // First call: GET check (returns 404 Not Found)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                send_mock_response(&mut stream, "404 Not Found", r#"{"message":"Not Found"}"#);
            }

            // Second call: POST create (returns 201 Created)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                send_mock_response(
                    &mut stream,
                    "201 Created",
                    r#"{"id":101,"html_url":"https://github.com/org/repo/releases/tag/v1.0.0"}"#,
                );
            }
        });

        let client = GitHubClient::new("github.com", "ghp_mocktoken")
            .with_base_url(format!("http://127.0.0.1:{port}"));

        let res = client
            .create_or_update_release("org", "repo", "v1.0.0", "Release v1.0.0", "Notes", true)
            .unwrap();

        assert_eq!(res.release_id, "101");
        assert_eq!(
            res.release_url,
            "https://github.com/org/repo/releases/tag/v1.0.0"
        );
        assert!(res.is_draft);
    }

    #[test]
    fn github_release_update_idempotent_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            // First call: GET check (returns 200 OK with existing ID 202)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                send_mock_response(
                    &mut stream,
                    "200 OK",
                    r#"{"id":202,"html_url":"https://github.com/org/repo/releases/tag/v2.0.0"}"#,
                );
            }

            // Second call: PATCH update (returns 200 OK)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                send_mock_response(
                    &mut stream,
                    "200 OK",
                    r#"{"id":202,"html_url":"https://github.com/org/repo/releases/tag/v2.0.0"}"#,
                );
            }
        });

        let client = GitHubClient::new("github.com", "ghp_mocktoken")
            .with_base_url(format!("http://127.0.0.1:{port}"));

        let res = client
            .create_or_update_release(
                "org",
                "repo",
                "v2.0.0",
                "Release v2.0.0",
                "Updated Notes",
                true,
            )
            .unwrap();

        assert_eq!(res.release_id, "202");
        assert_eq!(
            res.release_url,
            "https://github.com/org/repo/releases/tag/v2.0.0"
        );
    }

    #[test]
    fn github_handles_401_and_rate_limit_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 1024];
                let _ = stream.read(&mut buf);
                send_mock_response(
                    &mut stream,
                    "401 Unauthorized",
                    r#"{"message":"Bad credentials"}"#,
                );
            }
        });

        let client = GitHubClient::new("github.com", "bad_token")
            .with_base_url(format!("http://127.0.0.1:{port}"));

        let err = client
            .create_or_update_release("org", "repo", "v1.0.0", "Title", "Body", true)
            .unwrap_err();

        assert!(err.to_string().contains("401 Unauthorized"));
    }
}
