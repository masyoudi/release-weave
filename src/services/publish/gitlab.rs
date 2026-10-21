use std::time::Duration;

use serde_json::json;

use crate::errors::{AppError, Result};
use crate::services::publish::PublishedRelease;

pub struct GitLabClient {
    base_url: String,
    token: String,
    timeout: Duration,
}

impl GitLabClient {
    pub fn new(host: &str, token: &str) -> Self {
        let base_url = if host == "gitlab.com" {
            "https://gitlab.com".to_string()
        } else if host.starts_with("http://") || host.starts_with("https://") {
            host.trim_end_matches('/').to_string()
        } else {
            format!("https://{host}")
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

    fn encode_project_path(owner: &str, repo: &str) -> String {
        let full = format!("{owner}/{repo}");
        full.replace('/', "%2F")
    }

    pub fn create_or_update_release(
        &self,
        owner: &str,
        repo: &str,
        tag: &str,
        name: &str,
        description: &str,
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

        let encoded_id = Self::encode_project_path(owner, repo);
        let check_url = format!(
            "{}/api/v4/projects/{encoded_id}/releases/{tag}",
            self.base_url
        );
        let agent = self.agent();

        let resp = agent
            .get(&check_url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ReleaseWeave")
            .call()
            .map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    format!("GitLab connection error: {e}"),
                ))
            })?;

        let status = resp.status().as_u16();
        if status == 200 {
            // Already exists -> PUT /api/v4/projects/:id/releases/:tag_name
            let val: serde_json::Value = resp.into_body().read_json().unwrap_or_default();
            let web_url = val["_links"]["self"]
                .as_str()
                .or_else(|| val["upcoming_release_url"].as_str())
                .unwrap_or_default()
                .to_string();

            let update_url = format!(
                "{}/api/v4/projects/{encoded_id}/releases/{tag}",
                self.base_url
            );
            let put_resp = agent
                .put(&update_url)
                .header("PRIVATE-TOKEN", &self.token)
                .header("User-Agent", "ReleaseWeave")
                .send_json(json!({
                    "name": name,
                    "description": description,
                }))
                .map_err(|e| {
                    AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("GitLab update error: {e}"),
                    ))
                })?;

            let put_status = put_resp.status().as_u16();
            if put_status == 200 {
                let updated_val: serde_json::Value =
                    put_resp.into_body().read_json().unwrap_or_default();
                let final_url = updated_val["_links"]["self"]
                    .as_str()
                    .unwrap_or(&web_url)
                    .to_string();
                Ok(PublishedRelease {
                    provider: "GitLab".to_string(),
                    host: self.base_url.clone(),
                    release_url: final_url,
                    release_id: tag.to_string(),
                    tag_name: tag.to_string(),
                    is_draft: false,
                })
            } else {
                let msg = put_resp.into_body().read_to_string().unwrap_or_default();
                Err(Self::handle_error(put_status, &msg))
            }
        } else if status == 404 {
            // Does not exist -> POST /api/v4/projects/:id/releases
            let create_url = format!("{}/api/v4/projects/{encoded_id}/releases", self.base_url);
            let post_resp = agent
                .post(&create_url)
                .header("PRIVATE-TOKEN", &self.token)
                .header("User-Agent", "ReleaseWeave")
                .send_json(json!({
                    "name": name,
                    "tag_name": tag,
                    "description": description,
                }))
                .map_err(|e| {
                    AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("GitLab create error: {e}"),
                    ))
                })?;

            let post_status = post_resp.status().as_u16();
            if post_status == 201 {
                let created_val: serde_json::Value =
                    post_resp.into_body().read_json().map_err(|e| {
                        AppError::Validation(format!(
                            "Failed to parse created GitLab release response: {e}"
                        ))
                    })?;
                let web_url = created_val["_links"]["self"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                Ok(PublishedRelease {
                    provider: "GitLab".to_string(),
                    host: self.base_url.clone(),
                    release_url: web_url,
                    release_id: tag.to_string(),
                    tag_name: tag.to_string(),
                    is_draft: false,
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
                "GitLab authentication failed (401 Unauthorized): Token is invalid or expired. Please update token in Settings.".to_string(),
            ),
            403 => AppError::Validation(format!(
                "GitLab request forbidden (403): Token lacks Developer or Maintainer permissions: {msg}"
            )),
            404 => AppError::Validation(format!("GitLab project or tag not found (404): {msg}")),
            409 => AppError::Validation(format!("GitLab release conflict (409): {msg}")),
            422 => AppError::Validation(format!(
                "GitLab validation failed (422 Unprocessable Entity): {msg}"
            )),
            _ => AppError::Validation(format!("GitLab API error (HTTP {status}): {msg}")),
        }
    }

    pub fn test_connection(&self) -> Result<String> {
        let url = format!("{}/api/v4/user", self.base_url);
        let resp = self
            .agent()
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ReleaseWeave")
            .call()
            .map_err(|e| AppError::Validation(format!("GitLab connection error: {e}")))?;

        let status = resp.status().as_u16();
        if status == 200 {
            let val: serde_json::Value = resp.into_body().read_json().unwrap_or_default();
            let username = val["username"].as_str().unwrap_or("authenticated");
            Ok(format!("Connected as GitLab user: @{username}"))
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

    #[test]
    fn gitlab_release_create_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            // First call: GET check (returns 404)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"message\":\"404 Release Not Found\"}";
                let _ = stream.write_all(resp.as_bytes());
            }

            // Second call: POST create (returns 201)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let resp = "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"tag_name\":\"v1.0.0\",\"_links\":{\"self\":\"https://gitlab.com/group/repo/-/releases/v1.0.0\"}}";
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let client = GitLabClient::new("gitlab.com", "glpat-mock")
            .with_base_url(format!("http://127.0.0.1:{port}"));

        let res = client
            .create_or_update_release("group", "repo", "v1.0.0", "Release v1.0.0", "Notes")
            .unwrap();

        assert_eq!(res.release_id, "v1.0.0");
        assert_eq!(
            res.release_url,
            "https://gitlab.com/group/repo/-/releases/v1.0.0"
        );
    }

    #[test]
    fn gitlab_release_update_idempotent_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            // First call: GET check (returns 200 OK)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"tag_name\":\"v2.0.0\",\"_links\":{\"self\":\"https://gitlab.com/group/repo/-/releases/v2.0.0\"}}";
                let _ = stream.write_all(resp.as_bytes());
            }

            // Second call: PUT update (returns 200 OK)
            if let Some(stream) = listener.incoming().next() {
                let mut stream = stream.unwrap();
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"tag_name\":\"v2.0.0\",\"_links\":{\"self\":\"https://gitlab.com/group/repo/-/releases/v2.0.0\"}}";
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let client = GitLabClient::new("gitlab.com", "glpat-mock")
            .with_base_url(format!("http://127.0.0.1:{port}"));

        let res = client
            .create_or_update_release("group", "repo", "v2.0.0", "Release v2.0.0", "Updated")
            .unwrap();

        assert_eq!(res.release_id, "v2.0.0");
        assert_eq!(
            res.release_url,
            "https://gitlab.com/group/repo/-/releases/v2.0.0"
        );
    }
}
