use std::path::PathBuf;

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePatch {
    pub path: PathBuf,
    pub relative_path: String,
    pub old_content: Option<String>,
    pub new_content: String,
    pub old_sha256: Option<String>,
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

impl FilePatch {
    pub fn new(
        path: PathBuf,
        relative_path: String,
        old_content: Option<String>,
        new_content: String,
    ) -> Self {
        let old_sha256 = old_content.as_ref().map(|c| sha256_hex(c.as_bytes()));
        Self {
            path,
            relative_path,
            old_content,
            new_content,
            old_sha256,
        }
    }

    pub fn diff_summary(&self) -> String {
        let old_lines = self
            .old_content
            .as_deref()
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let new_lines = self.new_content.lines().count();
        format!(
            "{}: {} -> {} lines",
            self.relative_path, old_lines, new_lines
        )
    }

    pub fn to_unified_diff(&self) -> String {
        let old_bytes = self.old_content.as_deref().unwrap_or("").as_bytes();
        let new_bytes = self.new_content.as_bytes();
        let old_path = std::path::Path::new(&self.relative_path);
        let new_path = std::path::Path::new(&self.relative_path);

        if let Ok(mut patch) =
            git2::Patch::from_buffers(old_bytes, Some(old_path), new_bytes, Some(new_path), None)
        {
            let mut diff_bytes = Vec::new();
            let _ = patch.print(&mut |_, _, line| {
                let origin = line.origin();
                if origin == '+' || origin == '-' || origin == ' ' {
                    diff_bytes.push(origin as u8);
                }
                diff_bytes.extend_from_slice(line.content());
                true
            });
            if !diff_bytes.is_empty() {
                return String::from_utf8_lossy(&diff_bytes).to_string();
            }
        }

        format!(
            "--- a/{}\n+++ b/{}\n@@ file updated @@\n",
            self.relative_path, self.relative_path
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChangePlan {
    pub target_version: Version,
    pub patches: Vec<FilePatch>,
}

impl FileChangePlan {
    pub fn new(target_version: Version, patches: Vec<FilePatch>) -> Self {
        Self {
            target_version,
            patches,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_patch_computes_sha256() {
        let patch = FilePatch::new(
            PathBuf::from("/repo/Cargo.toml"),
            "Cargo.toml".into(),
            Some("old content".into()),
            "new content".into(),
        );

        assert!(patch.old_sha256.is_some());
        assert_eq!(patch.diff_summary(), "Cargo.toml: 1 -> 1 lines");
        let diff = patch.to_unified_diff();
        assert!(diff.contains("-old content"));
        assert!(diff.contains("+new content"));
    }
}
