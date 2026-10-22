use std::path::{Path, PathBuf};

use gpui_kit::Context;
use rusqlite::Connection;
use semver::Version;

use crate::ai::prompts::ContextReport;
use crate::domain::check::CheckResult;
use crate::domain::commit::{ChangeCategory, CommitInfo};
use crate::domain::file_change::FileChangePlan;
use crate::domain::package::Package;
use crate::domain::project::{Project, ProjectSettings};
use crate::domain::release::{ChangeItem, Release};
use crate::errors::Result;
use crate::services::git_service::CommitRange;
use crate::services::readiness_service::ReadinessSummary;
use crate::services::release_service::ReleaseService;
use crate::services::template::{TEMPLATE_KEEP_A_CHANGELOG, TEMPLATE_SIMPLE_MARKDOWN};
use crate::services::version_service::{BumpRecommendation, VersionConsistencyReport};
use crate::storage::database::open_path;
use crate::storage::repositories::{PackageRepo, ProjectRepo, ReleaseRepo, SettingsRepo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavTab {
    #[default]
    Overview,
    Changes,
    ReleaseNotes,
    Versioning,
    Readiness,
    History,
    Settings,
}

impl NavTab {
    pub const ALL: [NavTab; 7] = [
        NavTab::Overview,
        NavTab::Changes,
        NavTab::ReleaseNotes,
        NavTab::Versioning,
        NavTab::Readiness,
        NavTab::History,
        NavTab::Settings,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            NavTab::Overview => "Overview",
            NavTab::Changes => "Changes",
            NavTab::ReleaseNotes => "Release Notes",
            NavTab::Versioning => "Versioning",
            NavTab::Readiness => "Readiness",
            NavTab::History => "History",
            NavTab::Settings => "Settings",
        }
    }

    pub fn shortcut_key(&self) -> &'static str {
        match self {
            NavTab::Overview => "Ctrl+1",
            NavTab::Changes => "Ctrl+2",
            NavTab::ReleaseNotes => "Ctrl+3",
            NavTab::Versioning => "Ctrl+4",
            NavTab::Readiness => "Ctrl+5",
            NavTab::History => "Ctrl+6",
            NavTab::Settings => "Ctrl+7",
        }
    }

    pub fn icon(&self) -> gpui_kit::assets::IconName {
        match self {
            NavTab::Overview => gpui_kit::assets::IconName::LayoutDashboard,
            NavTab::Changes => gpui_kit::assets::IconName::GitCommitHorizontal,
            NavTab::ReleaseNotes => gpui_kit::assets::IconName::FileText,
            NavTab::Versioning => gpui_kit::assets::IconName::Tag,
            NavTab::Readiness => gpui_kit::assets::IconName::ShieldCheck,
            NavTab::History => gpui_kit::assets::IconName::Archive,
            NavTab::Settings => gpui_kit::assets::IconName::Settings,
        }
    }
}

pub struct AppState {
    pub db_path: PathBuf,
    pub projects: Vec<Project>,
    pub active_project: Option<Project>,
    pub active_nav: NavTab,

    pub commits: Vec<CommitInfo>,
    pub commit_range: Option<CommitRange>,
    pub change_items: Vec<ChangeItem>,
    pub readiness_checks: Vec<CheckResult>,
    pub readiness_summary: Option<ReadinessSummary>,
    pub release_notes: String,
    pub active_release: Option<Release>,
    pub history_releases: Vec<Release>,
    pub context_report: Option<ContextReport>,
    pub bump_recommendation: Option<BumpRecommendation>,
    pub version_consistency: Option<VersionConsistencyReport>,
    pub current_version: Option<String>,
    pub target_version: String,
    pub selected_category_filter: Option<ChangeCategory>,
    pub selected_commit_hash: Option<String>,
    pub diff_preview: Option<String>,
    pub diff_truncated: bool,
    pub selected_template_id: String,

    pub is_loading: bool,
    pub loading_message: Option<String>,
    pub error_message: Option<String>,
    pub status_notice: Option<String>,
    pub theme_dark: bool,
    pub global_settings: ProjectSettings,
    pub detected_stack: Option<crate::services::readiness_service::DetectedStack>,
    pub show_open_repo_dialog: bool,
    pub show_project_switcher: bool,
    pub show_finalize_confirm: bool,
    pub show_apply_plan_dialog: bool,
    pub show_tag_confirm: bool,
    pub show_command_palette: bool,
    pub command_palette_query: String,
    pub command_palette_selected_index: usize,
    pub pending_file_plan: Option<FileChangePlan>,
    pub selected_plan_patch_idx: usize,
    pub sidebar_collapsed: bool,
    pub packages: Vec<Package>,
    pub active_package_id: Option<i64>,
    pub show_package_selector: bool,
}

impl AppState {
    pub fn default_global_settings() -> ProjectSettings {
        ProjectSettings {
            ollama_url: Some("http://localhost:11434".to_string()),
            model: Some("qwen2.5-coder:7b".to_string()),
            context_limit: Some(12000),
            readiness_commands: Vec::new(),
            language: Some("en".to_string()),
            commands_required: false,
            allow_git_tag: false,
            package_tag_pattern: None,
        }
    }

    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            projects: Vec::new(),
            active_project: None,
            active_nav: NavTab::Overview,
            commits: Vec::new(),
            commit_range: None,
            change_items: Vec::new(),
            readiness_checks: Vec::new(),
            readiness_summary: None,
            release_notes: String::new(),
            active_release: None,
            history_releases: Vec::new(),
            context_report: None,
            bump_recommendation: None,
            version_consistency: None,
            current_version: None,
            target_version: String::new(),
            selected_category_filter: None,
            selected_commit_hash: None,
            diff_preview: None,
            diff_truncated: false,
            selected_template_id: "keep-a-changelog".to_string(),
            is_loading: false,
            loading_message: None,
            error_message: None,
            status_notice: None,
            theme_dark: true,
            global_settings: Self::default_global_settings(),
            detected_stack: None,
            show_open_repo_dialog: false,
            show_project_switcher: false,
            show_finalize_confirm: false,
            show_apply_plan_dialog: false,
            show_tag_confirm: false,
            show_command_palette: false,
            command_palette_query: String::new(),
            command_palette_selected_index: 0,
            pending_file_plan: None,
            selected_plan_patch_idx: 0,
            sidebar_collapsed: false,
            packages: Vec::new(),
            active_package_id: None,
            show_package_selector: false,
        }
    }

    pub fn effective_settings(&self) -> ProjectSettings {
        let mut s = self.global_settings.clone();
        if let Some(proj) = &self.active_project {
            if !proj.settings.readiness_commands.is_empty() {
                s.readiness_commands = proj.settings.readiness_commands.clone();
            }
            s.commands_required = proj.settings.commands_required;
            s.allow_git_tag = proj.settings.allow_git_tag;
            if proj.settings.package_tag_pattern.is_some() {
                s.package_tag_pattern = proj.settings.package_tag_pattern;
            }
        }
        s
    }

    pub fn update_project_settings_sync(&mut self, settings: ProjectSettings) -> Result<()> {
        if let Some(proj) = &mut self.active_project {
            proj.settings = settings.clone();
            if let Some(pid) = proj.id {
                let conn = self.open_db()?;
                ProjectRepo::update_settings(&conn, pid, &settings)?;
            }
        }
        Ok(())
    }

    pub fn active_template_str(&self) -> &'static str {
        match self.selected_template_id.as_str() {
            "simple-markdown" => TEMPLATE_SIMPLE_MARKDOWN,
            _ => TEMPLATE_KEEP_A_CHANGELOG,
        }
    }

    pub fn open_db(&self) -> Result<Connection> {
        open_path(&self.db_path)
    }

    pub fn set_nav(&mut self, nav: NavTab, cx: &mut Context<Self>) {
        self.active_nav = nav;
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_dark = !self.theme_dark;
        cx.notify();
    }

    pub fn set_loading(&mut self, loading: bool, message: Option<String>) {
        self.is_loading = loading;
        self.loading_message = message;
    }

    pub fn set_error(&mut self, err: Option<String>) {
        self.error_message = err;
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn set_notice(&mut self, notice: Option<String>) {
        self.status_notice = notice;
    }

    pub fn clear_notice(&mut self) {
        self.status_notice = None;
    }

    pub fn load_projects_sync(&mut self) -> Result<()> {
        let conn = self.open_db()?;
        if let Ok(saved) = SettingsRepo::get_global_settings(&conn) {
            let defaults = Self::default_global_settings();
            self.global_settings = ProjectSettings {
                ollama_url: saved.ollama_url.or(defaults.ollama_url),
                model: saved.model.or(defaults.model),
                context_limit: saved.context_limit.or(defaults.context_limit),
                readiness_commands: saved.readiness_commands,
                language: saved.language.or(defaults.language),
                commands_required: saved.commands_required,
                allow_git_tag: saved.allow_git_tag,
                package_tag_pattern: saved.package_tag_pattern.or(defaults.package_tag_pattern),
            };
        }
        self.projects = ProjectRepo::list(&conn)?;
        if self.active_project.is_none()
            && let Some(first) = self.projects.first().cloned()
        {
            self.active_project = Some(first.clone());
            self.detected_stack = Some(
                crate::services::readiness_service::ReadinessService::detect_project_stack(
                    &first.repository_path,
                ),
            );
            if let Some(pid) = first.id
                && let Ok(rels) = ReleaseRepo::list_by_project(&conn, pid)
            {
                self.history_releases = rels;
            }
        }
        Ok(())
    }

    pub fn save_global_settings_sync(&mut self, settings: ProjectSettings) -> Result<()> {
        let conn = self.open_db()?;
        SettingsRepo::set_global_settings(&conn, &settings)?;
        self.global_settings = settings;
        Ok(())
    }

    pub fn load_history_sync(&mut self) -> Result<Vec<Release>> {
        let Some(proj_id) = self.active_project.as_ref().and_then(|p| p.id) else {
            self.history_releases.clear();
            return Ok(Vec::new());
        };
        let conn = self.open_db()?;
        let rels = ReleaseRepo::list_by_project(&conn, proj_id)?;
        self.history_releases = rels.clone();
        Ok(rels)
    }

    pub fn select_commit_patch_sync(&mut self, commit_hash: &str) -> Result<()> {
        let Some(proj) = &self.active_project else {
            return Ok(());
        };
        let repo = git2::Repository::open(&proj.repository_path)?;
        let oid = git2::Oid::from_str(commit_hash)?;
        let res = crate::services::git_service::GitService::commit_patch(&repo, oid, 16_384)?;
        self.selected_commit_hash = Some(commit_hash.to_string());
        self.diff_preview = Some(res.patch);
        self.diff_truncated = res.truncated;
        Ok(())
    }

    pub fn toggle_open_repo_dialog(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_open_repo_dialog = show;
        if show {
            self.show_project_switcher = false;
        }
        cx.notify();
    }

    pub fn toggle_project_switcher(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_project_switcher = show;
        if show {
            self.show_open_repo_dialog = false;
        }
        cx.notify();
    }

    pub fn toggle_apply_plan_dialog(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_apply_plan_dialog = show;
        if !show {
            self.pending_file_plan = None;
            self.selected_plan_patch_idx = 0;
        }
        cx.notify();
    }

    pub fn toggle_command_palette(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_command_palette = show;
        if show {
            self.command_palette_query.clear();
            self.command_palette_selected_index = 0;
        }
        cx.notify();
    }

    pub fn toggle_package_selector(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_package_selector = show;
        cx.notify();
    }

    pub fn set_palette_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.command_palette_query = query;
        self.command_palette_selected_index = 0;
        cx.notify();
    }

    pub fn palette_select_next(&mut self, max_items: usize, cx: &mut Context<Self>) {
        if max_items > 0 && self.command_palette_selected_index + 1 < max_items {
            self.command_palette_selected_index += 1;
            cx.notify();
        }
    }

    pub fn palette_select_previous(&mut self, cx: &mut Context<Self>) {
        if self.command_palette_selected_index > 0 {
            self.command_palette_selected_index -= 1;
            cx.notify();
        }
    }

    pub fn execute_palette_action(
        &mut self,
        action: crate::ui::palette::registry::PaletteAction,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::palette::registry::PaletteAction;
        self.show_command_palette = false;
        match action {
            PaletteAction::Navigate(tab) => {
                self.set_nav(tab, cx);
            }
            PaletteAction::OpenRepoDialog => {
                self.toggle_open_repo_dialog(true, cx);
            }
            PaletteAction::ToggleProjectSwitcher => {
                self.toggle_project_switcher(true, cx);
            }
            PaletteAction::SwitchProject(project_id) => {
                if let Some(proj) = self
                    .projects
                    .iter()
                    .find(|p| p.id == Some(project_id))
                    .cloned()
                {
                    self.select_project(proj, cx);
                }
            }
            PaletteAction::SelectPackage(pkg_id) => {
                self.select_package(pkg_id, cx);
            }
            PaletteAction::RunReadinessChecks => {
                self.set_nav(NavTab::Readiness, cx);
                self.run_readiness_checks_sync(cx);
            }
            PaletteAction::ToggleTheme => {
                self.toggle_theme(cx);
            }
            PaletteAction::CopyReleaseNotes => {
                if !self.release_notes.is_empty() {
                    self.set_notice(Some("Release notes ready to copy.".to_string()));
                } else {
                    self.set_notice(Some("No release notes draft to copy yet.".to_string()));
                }
                cx.notify();
            }
            PaletteAction::ApplyFiles => {
                self.set_nav(NavTab::ReleaseNotes, cx);
                self.plan_file_changes_sync(cx);
            }
            PaletteAction::FinalizeRelease => {
                self.set_nav(NavTab::ReleaseNotes, cx);
                self.show_finalize_confirm = true;
                cx.notify();
            }
        }
    }

    pub fn run_readiness_checks_sync(&mut self, cx: &mut Context<Self>) {
        let Some(project) = &self.active_project else {
            self.set_error(Some("No active project selected".to_string()));
            cx.notify();
            return;
        };

        let repo_path = project.repository_path.clone();
        let target_ver = if self.target_version.is_empty() {
            "0.1.0".to_string()
        } else {
            self.target_version.clone()
        };
        let settings = self.effective_settings();

        self.run_background(
            cx,
            "Running release readiness checks...",
            move || {
                let repo = git2::Repository::open(&repo_path)?;
                let semver_target = semver::Version::parse(&target_ver)
                    .unwrap_or_else(|_| semver::Version::new(0, 1, 0));
                let summary = crate::services::readiness_service::ReadinessService::run_all(
                    &repo,
                    &repo_path,
                    &semver_target,
                    &settings,
                );
                let results = summary.checks.clone();
                Ok((results, summary))
            },
            |s, (results, summary), _| {
                s.readiness_checks = results;
                s.readiness_summary = Some(summary);
            },
        );
    }

    pub fn select_plan_patch(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.selected_plan_patch_idx = idx;
        cx.notify();
    }

    pub fn plan_file_changes_sync(&mut self, cx: &mut Context<Self>) {
        let Some(ref project) = self.active_project else {
            self.set_error(Some("No active project selected".to_string()));
            cx.notify();
            return;
        };

        let ver_str = if self.target_version.is_empty() {
            "0.1.0"
        } else {
            &self.target_version
        };

        let ver = match Version::parse(ver_str) {
            Ok(v) => v,
            Err(e) => {
                self.set_error(Some(format!("Invalid target version '{ver_str}': {e}")));
                cx.notify();
                return;
            }
        };

        match ReleaseService::plan_apply(&project.repository_path, &ver, &self.release_notes) {
            Ok(plan) => {
                if plan.is_empty() {
                    self.set_notice(Some("No manifest or changelog files to update".to_string()));
                    cx.notify();
                    return;
                }
                self.pending_file_plan = Some(plan);
                self.selected_plan_patch_idx = 0;
                self.show_apply_plan_dialog = true;
                cx.notify();
            }
            Err(e) => {
                self.set_error(Some(format!("Failed to plan file changes: {e}")));
                cx.notify();
            }
        }
    }

    pub fn apply_pending_file_plan_sync(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.pending_file_plan.take() else {
            self.show_apply_plan_dialog = false;
            cx.notify();
            return;
        };

        match ReleaseService::apply_plan(&plan) {
            Ok(modified_paths) => {
                let count = modified_paths.len();
                let files_list = modified_paths
                    .iter()
                    .filter_map(|p| p.file_name().and_then(|f| f.to_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.show_apply_plan_dialog = false;
                self.set_notice(Some(format!(
                    "Applied release v{} to {count} file(s): {files_list}",
                    plan.target_version
                )));
                cx.notify();
            }
            Err(e) => {
                self.pending_file_plan = Some(plan);
                self.set_error(Some(format!("Apply rejected: {e}")));
                cx.notify();
            }
        }
    }

    pub fn create_git_tag_sync(&mut self, cx: &mut Context<Self>) {
        let Some(ref project) = self.active_project else {
            self.set_error(Some("No active project selected".to_string()));
            cx.notify();
            return;
        };

        let tag_name = if let Some(pkg_id) = self.active_package_id {
            if let Some(pkg) = self.packages.iter().find(|p| p.id == Some(pkg_id)) {
                let pat = project
                    .settings
                    .package_tag_pattern
                    .unwrap_or(crate::domain::project::PackageTagPattern::At);
                pat.format(&pkg.name, &self.target_version)
            } else if self.target_version.starts_with('v') {
                self.target_version.clone()
            } else {
                format!("v{}", self.target_version)
            }
        } else if self.target_version.starts_with('v') {
            self.target_version.clone()
        } else {
            format!("v{}", self.target_version)
        };

        let repo_path = project.repository_path.clone();
        let notes = self.release_notes.clone();

        match git2::Repository::open(&repo_path) {
            Ok(repo) => {
                match crate::services::git_service::GitService::create_annotated_tag(
                    &repo, &tag_name, &notes,
                ) {
                    Ok(tag_oid) => {
                        self.show_tag_confirm = false;
                        let short_oid: String = tag_oid.chars().take(7).collect();
                        self.set_notice(Some(format!(
                            "Annotated tag '{tag_name}' created on HEAD ({short_oid})."
                        )));
                        cx.notify();
                    }
                    Err(e) => {
                        self.show_tag_confirm = false;
                        self.set_error(Some(format!("Failed to create Git tag: {e}")));
                        cx.notify();
                    }
                }
            }
            Err(e) => {
                self.show_tag_confirm = false;
                self.set_error(Some(format!("Cannot open repository: {e}")));
                cx.notify();
            }
        }
    }

    pub fn publish_draft_release_sync(&mut self, cx: &mut Context<Self>) {
        let Some(ref project) = self.active_project else {
            self.set_error(Some("No active project selected".to_string()));
            cx.notify();
            return;
        };

        let Some(ref release) = self.active_release else {
            self.set_error(Some("No active release to publish".to_string()));
            cx.notify();
            return;
        };

        if project.id.is_none() || Some(release.project_id) != project.id {
            self.active_release = None;
            self.set_error(Some(
                "Active release does not belong to the selected project".to_string(),
            ));
            cx.notify();
            return;
        }

        if release.status != crate::domain::release::ReleaseStatus::Final {
            self.set_error(Some(
                "Release must be finalized before publishing".to_string(),
            ));
            cx.notify();
            return;
        }

        let repo_path = project.repository_path.clone();
        let release_clone = release.clone();
        let db_path = self.db_path.clone();

        self.run_background(
            cx,
            "Publishing draft release to remote...",
            move || {
                let repo = git2::Repository::open(&repo_path)?;
                let remote_info =
                    crate::services::remote_service::RemoteService::detect_remote(&repo, None)?;
                let token = crate::security::secrets::SecretService::get_token(&remote_info.host)?
                    .ok_or_else(|| {
                        crate::errors::AppError::Validation(format!(
                            "No token found for host '{}'. Please configure token in Settings.",
                            remote_info.host
                        ))
                    })?;

                let published = crate::services::publish::PublishService::publish_draft(
                    &remote_info,
                    &token,
                    &release_clone,
                )?;

                if let Some(rid) = release_clone.id {
                    let conn = crate::storage::database::open_path(&db_path)?;
                    let mut meta_val = if let Some(m) = &release_clone.model_metadata {
                        serde_json::from_str::<serde_json::Value>(m)
                            .unwrap_or(serde_json::json!({ "model": m }))
                    } else {
                        serde_json::json!({})
                    };
                    meta_val["published_url"] =
                        serde_json::Value::String(published.release_url.clone());
                    meta_val["published_id"] =
                        serde_json::Value::String(published.release_id.clone());
                    meta_val["published_at"] =
                        serde_json::Value::String(chrono::Utc::now().to_rfc3339());

                    let meta_str = serde_json::to_string(&meta_val).unwrap_or_default();
                    crate::storage::repositories::ReleaseRepo::update_metadata(
                        &conn, rid, &meta_str,
                    )?;
                }

                Ok(published)
            },
            |s, pub_info, _| {
                if let Some(ref mut rel) = s.active_release {
                    let mut meta_val = if let Some(m) = &rel.model_metadata {
                        serde_json::from_str::<serde_json::Value>(m)
                            .unwrap_or(serde_json::json!({ "model": m }))
                    } else {
                        serde_json::json!({})
                    };
                    meta_val["published_url"] =
                        serde_json::Value::String(pub_info.release_url.clone());
                    rel.model_metadata = Some(serde_json::to_string(&meta_val).unwrap_or_default());
                }
                s.set_notice(Some(format!(
                    "Draft release published to {}: {}",
                    pub_info.provider, pub_info.release_url
                )));
            },
        );
    }

    pub fn select_project_sync(&mut self, project: Project) {
        self.active_project = Some(project.clone());
        self.show_project_switcher = false;
        self.show_open_repo_dialog = false;
        self.show_finalize_confirm = false;
        self.show_apply_plan_dialog = false;
        self.show_tag_confirm = false;
        self.show_command_palette = false;
        self.show_package_selector = false;
        self.pending_file_plan = None;
        self.selected_plan_patch_idx = 0;
        self.commits.clear();
        self.commit_range = None;
        self.change_items.clear();
        self.readiness_checks.clear();
        self.readiness_summary = None;
        self.release_notes.clear();
        self.active_release = None;
        self.context_report = None;
        self.bump_recommendation = None;
        self.version_consistency = None;
        self.current_version = None;
        self.target_version.clear();
        self.selected_category_filter = None;
        self.selected_commit_hash = None;
        self.diff_preview = None;
        self.diff_truncated = false;
        self.history_releases.clear();
        self.packages.clear();
        self.active_package_id = None;
        self.detected_stack = None;

        let repo_path = project.repository_path.clone();

        if let Ok(members) =
            crate::services::workspace_service::WorkspaceService::detect_workspace(&repo_path)
            && !members.is_empty()
        {
            if let Some(pid) = project.id
                && let Ok(conn) = self.open_db()
            {
                for m in &members {
                    let pkg = Package::new(
                        pid,
                        &m.name,
                        &m.relative_path,
                        &m.manifest_path,
                        m.current_version.clone(),
                    );
                    let _ = PackageRepo::upsert(&conn, &pkg);
                }
                if let Ok(db_pkgs) = PackageRepo::list_by_project(&conn, pid) {
                    self.packages = db_pkgs;
                }
            } else {
                self.packages = members
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let mut p = Package::new(
                            0,
                            m.name,
                            m.relative_path,
                            m.manifest_path,
                            m.current_version,
                        );
                        p.id = Some((i + 1) as i64);
                        p
                    })
                    .collect();
            }
        }

        if let Some(pid) = project.id
            && let Ok(conn) = self.open_db()
            && let Ok(rels) = ReleaseRepo::list_by_project(&conn, pid)
        {
            self.history_releases = rels;
        }

        // Perform fast sync scan for the selected repository
        let detected =
            crate::services::readiness_service::ReadinessService::detect_project_stack(&repo_path);
        self.detected_stack = Some(detected);
        if let Ok(repo) = git2::Repository::open(&repo_path) {
            let cur_opt = crate::services::version_service::VersionService::read_current_version(
                &repo, &repo_path,
            )
            .ok();
            if let Some(cur) = &cur_opt {
                self.current_version = Some(cur.to_string());
                self.target_version = cur.to_string();
            }
            if let Ok(manifest_opt) =
                crate::services::version_service::VersionService::read_manifest_version(&repo_path)
            {
                let manifests: Vec<_> = manifest_opt.into_iter().collect();
                self.version_consistency = Some(
                    crate::services::version_service::VersionService::check_consistency(
                        cur_opt.as_ref(),
                        &manifests,
                    ),
                );
            }
            if let Ok(range) =
                crate::services::git_service::GitService::resolve_range(&repo, None, "HEAD")
                && let Ok(commits) =
                    crate::services::git_service::GitService::commits_in_range(&repo, &range)
            {
                let mapper = crate::services::conventional::CategoryMapper::default();
                if let Ok(items) =
                    crate::services::git_service::GitService::generate_deterministic_change_items(
                        &repo, &range, &mapper,
                    )
                {
                    if let Some(cur) = &cur_opt {
                        let rec = crate::services::version_service::VersionService::recommend_bump(
                            cur, &commits,
                        );
                        self.target_version = rec.recommended_version.to_string();
                        self.bump_recommendation = Some(rec);
                    }
                    self.commits = commits;
                    self.commit_range = Some(range);
                    self.change_items = items;
                }
            }
        }
    }

    pub fn select_project(&mut self, project: Project, cx: &mut Context<Self>) {
        self.select_project_sync(project);
        cx.notify();
    }

    pub fn select_package(&mut self, package_id: Option<i64>, cx: &mut Context<Self>) {
        self.active_package_id = package_id;
        self.show_package_selector = false;
        self.selected_commit_hash = None;
        self.diff_preview = None;

        let Some(ref project) = self.active_project else {
            cx.notify();
            return;
        };

        let repo_path = project.repository_path.clone();
        if let Ok(repo) = git2::Repository::open(&repo_path) {
            let mapper = crate::services::conventional::CategoryMapper::default();
            let range =
                crate::services::git_service::GitService::resolve_range(&repo, None, "HEAD").ok();

            if let Some(pkg_id) = package_id {
                if let Some(pkg) = self.packages.iter().find(|p| p.id == Some(pkg_id)).cloned() {
                    let cur_ver =
                        crate::services::version_service::VersionService::read_package_current_version(
                            &repo, &pkg, &repo_path,
                        )
                        .unwrap_or_else(|_| Version::new(0, 1, 0));

                    self.current_version = Some(cur_ver.to_string());
                    self.target_version = cur_ver.to_string();

                    if let Some(ref r) = range {
                        if let Ok(commits) =
                            crate::services::git_service::GitService::commits_in_range_with_path(
                                &repo,
                                r,
                                Some(&pkg.relative_path),
                            )
                        {
                            let rec =
                                crate::services::version_service::VersionService::recommend_bump(
                                    &cur_ver, &commits,
                                );
                            self.target_version = rec.recommended_version.to_string();
                            self.bump_recommendation = Some(rec);
                            self.commits = commits;
                        }
                        if let Ok(items) =
                            crate::services::git_service::GitService::generate_deterministic_change_items_with_path(
                                &repo,
                                r,
                                &mapper,
                                Some(&pkg.relative_path),
                            )
                        {
                            self.change_items = items;
                        }
                    }

                    if let Some(pid) = project.id
                        && let Ok(conn) = self.open_db()
                        && let Ok(rels) = ReleaseRepo::list_by_package(&conn, pid, pkg_id)
                    {
                        self.history_releases = rels;
                    }
                }
            } else {
                let cur_opt =
                    crate::services::version_service::VersionService::read_current_version(
                        &repo, &repo_path,
                    )
                    .ok();
                if let Some(cur) = &cur_opt {
                    self.current_version = Some(cur.to_string());
                    self.target_version = cur.to_string();
                }

                if let Some(ref r) = range {
                    if let Ok(commits) =
                        crate::services::git_service::GitService::commits_in_range(&repo, r)
                    {
                        if let Some(cur) = &cur_opt {
                            let rec =
                                crate::services::version_service::VersionService::recommend_bump(
                                    cur, &commits,
                                );
                            self.target_version = rec.recommended_version.to_string();
                            self.bump_recommendation = Some(rec);
                        }
                        self.commits = commits;
                    }
                    if let Ok(items) =
                        crate::services::git_service::GitService::generate_deterministic_change_items(
                            &repo, r, &mapper,
                        )
                    {
                        self.change_items = items;
                    }
                }

                if let Some(pid) = project.id
                    && let Ok(conn) = self.open_db()
                    && let Ok(rels) = ReleaseRepo::list_by_project(&conn, pid)
                {
                    self.history_releases = rels;
                }
            }
        }

        cx.notify();
    }

    pub fn open_repository_path(&mut self, path: &Path, cx: &mut Context<Self>) -> Result<()> {
        let valid_path = crate::security::path_validation::validate_dir(path)?;
        let repo = git2::Repository::open(&valid_path)?;

        let default_branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().ok().map(|s| s.to_string()))
            .unwrap_or_else(|| "main".to_string());

        let name = valid_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        let conn = self.open_db()?;
        let existing = self
            .projects
            .iter()
            .find(|p| p.repository_path == valid_path);
        let project = match existing {
            Some(p) => p.clone(),
            None => {
                let mut new_proj = Project::new(&name, &valid_path, &default_branch);
                let detected =
                    crate::services::readiness_service::ReadinessService::detect_project_stack(
                        &valid_path,
                    );
                new_proj.settings.readiness_commands = detected.suggested_commands;
                let id = ProjectRepo::insert(&conn, &new_proj)?;
                new_proj.id = Some(id);
                self.projects.insert(0, new_proj.clone());
                new_proj
            }
        };

        self.select_project(project, cx);
        Ok(())
    }

    pub fn remove_project_sync(&mut self, project_id: i64) -> Result<bool> {
        let conn = self.open_db()?;
        ProjectRepo::remove(&conn, project_id)?;
        let was_active = self.active_project.as_ref().and_then(|p| p.id) == Some(project_id);
        self.projects.retain(|p| p.id != Some(project_id));
        Ok(was_active)
    }

    pub fn remove_project(&mut self, project_id: i64, cx: &mut Context<Self>) {
        match self.remove_project_sync(project_id) {
            Ok(was_active) => {
                if self.projects.is_empty() {
                    self.active_project = None;
                    self.active_release = None;
                    self.context_report = None;
                    self.version_consistency = None;
                    self.detected_stack = None;
                    self.history_releases.clear();
                    self.packages.clear();
                    self.active_package_id = None;
                    self.selected_commit_hash = None;
                    self.diff_preview = None;
                    self.diff_truncated = false;
                    self.selected_category_filter = None;
                    self.show_project_switcher = false;
                    self.commits.clear();
                    self.commit_range = None;
                    self.change_items.clear();
                    self.readiness_checks.clear();
                    self.readiness_summary = None;
                    self.release_notes.clear();
                    self.bump_recommendation = None;
                    self.current_version = None;
                    self.target_version.clear();
                } else if was_active && let Some(next_proj) = self.projects.first().cloned() {
                    let keep_switcher = self.show_project_switcher;
                    let keep_open_dialog = self.show_open_repo_dialog;
                    self.select_project(next_proj, cx);
                    self.show_project_switcher = keep_switcher;
                    self.show_open_repo_dialog = keep_open_dialog;
                }
                cx.notify();
            }
            Err(e) => {
                self.set_error(Some(e.to_string()));
                cx.notify();
            }
        }
    }

    pub fn open_current_dir(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let cwd = std::env::current_dir()?;
        self.open_repository_path(&cwd, cx)
    }

    pub fn pick_and_open_directory(&mut self, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        let initial_dir = std::env::current_dir().ok();

        cx.spawn(async move |entity, cx| {
            let picked = executor
                .spawn(async move {
                    let mut dialog =
                        rfd::FileDialog::new().set_title("Select Git Repository Directory");
                    if let Some(dir) = initial_dir {
                        dialog = dialog.set_directory(dir);
                    }
                    dialog.pick_folder()
                })
                .await;

            if let Some(path) = picked {
                _ = entity.update(cx, |this, cx| {
                    this.toggle_open_repo_dialog(false, cx);
                    this.toggle_project_switcher(false, cx);
                    if let Err(e) = this.open_repository_path(&path, cx) {
                        this.set_error(Some(e.to_string()));
                    }
                });
            }
        })
        .detach();
    }

    pub fn run_background<F, T>(
        &mut self,
        cx: &mut Context<Self>,
        loading_message: &str,
        task: F,
        on_success: impl Fn(&mut Self, T, &mut Context<Self>) + 'static,
    ) where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        self.set_loading(true, Some(loading_message.to_string()));
        self.error_message = None;
        cx.notify();

        let executor = cx.background_executor().clone();
        cx.spawn(async move |entity, cx| {
            let result = executor.spawn(async move { task() }).await;

            _ = entity.update(cx, |this, cx| {
                this.set_loading(false, None);
                match result {
                    Ok(val) => {
                        on_success(this, val, cx);
                    }
                    Err(e) => {
                        this.set_error(Some(e.to_string()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn state_initializes_with_defaults() {
        let dir = tempdir().unwrap();
        let db_file = dir.path().join("rw.db");
        let state = AppState::new(db_file);
        assert_eq!(state.active_nav, NavTab::Overview);
        assert!(!state.is_loading);
        assert!(state.error_message.is_none());
        assert_eq!(NavTab::ReleaseNotes.shortcut_key(), "Ctrl+3");
    }

    #[test]
    fn navigation_tabs_have_correct_titles_and_shortcuts() {
        assert_eq!(NavTab::Overview.title(), "Overview");
        assert_eq!(NavTab::Overview.shortcut_key(), "Ctrl+1");
        assert_eq!(NavTab::Changes.title(), "Changes");
        assert_eq!(NavTab::Changes.shortcut_key(), "Ctrl+2");
        assert_eq!(NavTab::ReleaseNotes.title(), "Release Notes");
        assert_eq!(NavTab::ReleaseNotes.shortcut_key(), "Ctrl+3");
        assert_eq!(NavTab::Versioning.title(), "Versioning");
        assert_eq!(NavTab::Versioning.shortcut_key(), "Ctrl+4");
        assert_eq!(NavTab::Readiness.title(), "Readiness");
        assert_eq!(NavTab::Readiness.shortcut_key(), "Ctrl+5");
        assert_eq!(NavTab::History.title(), "History");
        assert_eq!(NavTab::History.shortcut_key(), "Ctrl+6");
        assert_eq!(NavTab::Settings.title(), "Settings");
        assert_eq!(NavTab::Settings.shortcut_key(), "Ctrl+7");
    }

    #[test]
    fn loading_and_error_state_mutations() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().join("rw.db"));

        state.set_loading(true, Some("Analyzing Git...".to_string()));
        assert!(state.is_loading);
        assert_eq!(state.loading_message.as_deref(), Some("Analyzing Git..."));

        state.set_error(Some("Network timeout".to_string()));
        assert_eq!(state.error_message.as_deref(), Some("Network timeout"));

        state.set_loading(false, None);
        assert!(!state.is_loading);
        assert!(state.loading_message.is_none());
    }

    #[test]
    fn loads_projects_sync_from_database() {
        let dir = tempdir().unwrap();
        let db_file = dir.path().join("rw.db");
        let mut state = AppState::new(db_file);

        let conn = state.open_db().unwrap();
        let proj = Project::new("MyProject", dir.path(), "main");
        ProjectRepo::insert(&conn, &proj).unwrap();

        state.load_projects_sync().unwrap();
        assert_eq!(state.projects.len(), 1);
        assert_eq!(state.active_project.as_ref().unwrap().name, "MyProject");

        let proj_id = state.projects[0].id.unwrap();
        let was_active = state.remove_project_sync(proj_id).unwrap();
        assert!(was_active);
        assert!(state.projects.is_empty());
    }

    #[test]
    fn dialog_flags_toggle_properly() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().join("rw.db"));
        assert!(!state.show_open_repo_dialog);
        assert!(!state.show_project_switcher);

        state.show_open_repo_dialog = true;
        assert!(state.show_open_repo_dialog);
        state.show_project_switcher = true;
        assert!(state.show_project_switcher);
        state.show_tag_confirm = true;
        assert!(state.show_tag_confirm);
        state.show_command_palette = true;
        assert!(state.show_command_palette);
    }

    #[test]
    fn persists_settings_and_loads_release_history() {
        let dir = tempdir().unwrap();
        let db_file = dir.path().join("rw.db");
        let mut state = AppState::new(db_file.clone());

        let mut custom_settings = AppState::default_global_settings();
        custom_settings.model = Some("llama3.2:3b".to_string());
        custom_settings.context_limit = Some(8000);
        state.save_global_settings_sync(custom_settings).unwrap();

        let conn = state.open_db().unwrap();
        let proj = Project::new("RelProject", dir.path(), "main");
        let proj_id = ProjectRepo::insert(&conn, &proj).unwrap();
        let mut rel = crate::domain::release::Release::new_draft(
            proj_id,
            "1.0.0",
            Some("0.9.0".to_string()),
            Some("abc".to_string()),
            "def",
        );
        rel.status = crate::domain::release::ReleaseStatus::Final;
        rel.release_notes = "# v1.0.0\n\nInitial release".to_string();
        ReleaseRepo::insert(&conn, &rel).unwrap();

        let mut reloaded = AppState::new(db_file);
        reloaded.selected_template_id = "simple-markdown".to_string();
        reloaded.load_projects_sync().unwrap();
        assert_eq!(
            reloaded.global_settings.model.as_deref(),
            Some("llama3.2:3b")
        );
        assert_eq!(reloaded.global_settings.context_limit, Some(8000));
        assert!(
            reloaded
                .active_template_str()
                .contains("# Release {{version}}")
        );
        assert_eq!(reloaded.history_releases.len(), 1);
        assert_eq!(reloaded.history_releases[0].version, "1.0.0");

        // Verify project-level readiness commands override and update
        let mut proj_settings = reloaded.effective_settings();
        proj_settings.readiness_commands = vec!["npm test".to_string()];
        proj_settings.commands_required = true;
        proj_settings.allow_git_tag = true;
        reloaded
            .update_project_settings_sync(proj_settings)
            .unwrap();

        let effective = reloaded.effective_settings();
        assert_eq!(effective.readiness_commands, vec!["npm test".to_string()]);
        assert!(effective.commands_required);
        assert!(effective.allow_git_tag);
    }

    #[test]
    fn package_state_management() {
        let dir = tempdir().unwrap();
        let db_file = dir.path().join("rw.db");
        let mut state = AppState::new(db_file);

        assert!(state.packages.is_empty());
        assert_eq!(state.active_package_id, None);
        assert!(!state.show_package_selector);

        let pkg1 = Package::new(
            1,
            "pkg-a",
            "crates/pkg-a",
            "crates/pkg-a/Cargo.toml",
            Some("0.1.0".to_string()),
        );
        let mut pkg1_with_id = pkg1;
        pkg1_with_id.id = Some(10);

        state.packages.push(pkg1_with_id);
        state.active_package_id = Some(10);
        state.show_package_selector = true;

        assert_eq!(state.packages.len(), 1);
        assert_eq!(state.active_package_id, Some(10));
        assert!(state.show_package_selector);
    }

    #[test]
    fn project_switch_clears_active_release_and_prevents_cross_project_leak() {
        let dir = tempdir().unwrap();
        let db_file = dir.path().join("rw.db");
        let mut state = AppState::new(db_file);

        let p1 = Project::new("ProjOne", "/tmp/p1", "main");
        let mut p1_saved = p1;
        p1_saved.id = Some(1);

        let p2 = Project::new("ProjTwo", "/tmp/p2", "main");
        let mut p2_saved = p2;
        p2_saved.id = Some(2);

        state.active_project = Some(p1_saved);
        let mut rel = Release::new_draft(1, "1.0.0", None, None, "end");
        rel.id = Some(101);
        rel.status = crate::domain::release::ReleaseStatus::Final;
        rel.release_notes = "ProjOne finalized release notes".to_string();

        state.active_release = Some(rel);
        state.release_notes = "ProjOne finalized release notes".to_string();
        state.target_version = "1.0.0".to_string();

        assert!(state.active_release.is_some());
        assert_eq!(state.release_notes, "ProjOne finalized release notes");

        // Switch project to ProjTwo
        state.select_project_sync(p2_saved);

        // Verification: active release must be cleared, notes cleared, target version reset
        assert_eq!(state.active_project.as_ref().unwrap().name, "ProjTwo");
        assert!(state.active_release.is_none());
        assert!(state.release_notes.is_empty());
        assert_ne!(state.target_version, "1.0.0");
    }
}
