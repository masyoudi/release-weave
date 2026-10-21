pub mod github;
pub mod gitlab;

use crate::domain::release::Release;
use crate::errors::{AppError, Result};
use crate::services::remote_service::{RemoteInfo, RemoteProvider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedRelease {
    pub provider: String,
    pub host: String,
    pub release_url: String,
    pub release_id: String,
    pub tag_name: String,
    pub is_draft: bool,
}

pub struct PublishService;

impl PublishService {
    pub fn publish_draft(
        remote: &RemoteInfo,
        token: &str,
        release: &Release,
    ) -> Result<PublishedRelease> {
        let tag = format!("v{}", release.version);
        let title = format!("Release v{}", release.version);

        match remote.provider {
            RemoteProvider::GitHub => {
                let client = github::GitHubClient::new(&remote.host, token);
                client.create_or_update_release(
                    &remote.owner_path,
                    &remote.repo,
                    &tag,
                    &title,
                    &release.release_notes,
                    true,
                )
            }
            RemoteProvider::GitLab => {
                let client = gitlab::GitLabClient::new(&remote.host, token);
                client.create_or_update_release(
                    &remote.owner_path,
                    &remote.repo,
                    &tag,
                    &title,
                    &release.release_notes,
                )
            }
            RemoteProvider::Custom(ref host) => Err(AppError::Validation(format!(
                "Publishing to custom provider '{host}' is not currently supported."
            ))),
        }
    }
}
