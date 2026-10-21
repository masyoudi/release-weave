use gpui_kit::assets::IconName;

use crate::domain::package::Package;
use crate::domain::project::Project;
use crate::ui::state::NavTab;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    Navigate(NavTab),
    OpenRepoDialog,
    ToggleProjectSwitcher,
    SwitchProject(i64),
    SelectPackage(Option<i64>),
    RunReadinessChecks,
    ToggleTheme,
    CopyReleaseNotes,
    ApplyFiles,
    FinalizeRelease,
}

#[derive(Clone, Debug)]
pub struct PaletteCommand {
    pub id: String,
    pub title: String,
    pub category: String,
    pub keywords: Vec<String>,
    pub icon: Option<IconName>,
    pub action: PaletteAction,
}

impl PaletteCommand {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        category: impl Into<String>,
        keywords: &[&str],
        icon: Option<IconName>,
        action: PaletteAction,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            category: category.into(),
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            icon,
            action,
        }
    }
}

/// Builds the complete list of static and dynamic palette commands.
pub fn build_palette_commands(
    recent_projects: &[Project],
    packages: &[Package],
) -> Vec<PaletteCommand> {
    let mut commands = vec![
        // Navigation Commands
        PaletteCommand::new(
            "nav_overview",
            "Go to Overview",
            "Navigation",
            &["overview", "home", "dashboard"],
            Some(IconName::LayoutDashboard),
            PaletteAction::Navigate(NavTab::Overview),
        ),
        PaletteCommand::new(
            "nav_changes",
            "Go to Changes",
            "Navigation",
            &["changes", "commits", "diff", "log"],
            Some(IconName::GitCommitHorizontal),
            PaletteAction::Navigate(NavTab::Changes),
        ),
        PaletteCommand::new(
            "nav_release_notes",
            "Go to Release Notes",
            "Navigation",
            &["notes", "release notes", "changelog", "editor", "draft"],
            Some(IconName::FileText),
            PaletteAction::Navigate(NavTab::ReleaseNotes),
        ),
        PaletteCommand::new(
            "nav_versioning",
            "Go to Versioning",
            "Navigation",
            &["versioning", "bump", "semver", "tag"],
            Some(IconName::Tag),
            PaletteAction::Navigate(NavTab::Versioning),
        ),
        PaletteCommand::new(
            "nav_readiness",
            "Go to Readiness Checks",
            "Navigation",
            &["readiness", "checks", "verify", "gates", "rules"],
            Some(IconName::ShieldCheck),
            PaletteAction::Navigate(NavTab::Readiness),
        ),
        PaletteCommand::new(
            "nav_history",
            "Go to Release History",
            "Navigation",
            &["history", "releases", "archive", "past"],
            Some(IconName::Archive),
            PaletteAction::Navigate(NavTab::History),
        ),
        PaletteCommand::new(
            "nav_settings",
            "Go to Settings",
            "Navigation",
            &["settings", "config", "preferences", "tokens", "credentials"],
            Some(IconName::Settings),
            PaletteAction::Navigate(NavTab::Settings),
        ),
        // Action Commands
        PaletteCommand::new(
            "action_open_repo",
            "Open Repository...",
            "Repository",
            &["open", "repo", "folder", "git", "directory"],
            Some(IconName::FolderOpen),
            PaletteAction::OpenRepoDialog,
        ),
        PaletteCommand::new(
            "action_switch_repo",
            "Switch Repository...",
            "Repository",
            &["switch", "project", "recent", "change repo"],
            Some(IconName::FolderGit2),
            PaletteAction::ToggleProjectSwitcher,
        ),
        PaletteCommand::new(
            "action_run_readiness",
            "Run Readiness Checks",
            "Readiness",
            &[
                "readiness",
                "rdy",
                "checks",
                "run",
                "verify",
                "test",
                "validate",
            ],
            Some(IconName::Play),
            PaletteAction::RunReadinessChecks,
        ),
        PaletteCommand::new(
            "action_toggle_theme",
            "Toggle Color Theme",
            "Preferences",
            &["theme", "dark", "light", "mode", "color", "toggle"],
            Some(IconName::Sun),
            PaletteAction::ToggleTheme,
        ),
        PaletteCommand::new(
            "action_copy_notes",
            "Copy Release Notes",
            "Release",
            &["copy", "clipboard", "notes", "markdown"],
            Some(IconName::Copy),
            PaletteAction::CopyReleaseNotes,
        ),
        PaletteCommand::new(
            "action_apply_files",
            "Apply Target Version to Project Files",
            "Release",
            &["apply", "files", "manifest", "changelog", "patch"],
            Some(IconName::FileText),
            PaletteAction::ApplyFiles,
        ),
        PaletteCommand::new(
            "action_finalize_release",
            "Finalize Release",
            "Release",
            &["finalize", "lock", "done", "seal"],
            Some(IconName::CheckCheck),
            PaletteAction::FinalizeRelease,
        ),
    ];

    // Dynamic Repository Commands (T-204)
    for project in recent_projects {
        if let Some(id) = project.id {
            commands.push(PaletteCommand::new(
                format!("switch_repo_{id}"),
                format!("Switch to {}", project.name),
                "Repository",
                &["switch", "project", "repo", &project.name.to_lowercase()],
                Some(IconName::FolderGit2),
                PaletteAction::SwitchProject(id),
            ));
        }
    }

    // Dynamic Package Commands (T-214)
    if !packages.is_empty() {
        commands.push(PaletteCommand::new(
            "pkg_select_all",
            "Package: All Packages (Project Root)",
            "Package",
            &["package", "all", "root", "workspace", "monorepo"],
            Some(IconName::Boxes),
            PaletteAction::SelectPackage(None),
        ));
        for pkg in packages {
            if let Some(id) = pkg.id {
                commands.push(PaletteCommand::new(
                    format!("pkg_select_{id}"),
                    format!("Package: {}", pkg.name),
                    "Package",
                    &["package", "mono", "subproject", &pkg.name.to_lowercase()],
                    Some(IconName::Package),
                    PaletteAction::SelectPackage(Some(id)),
                ));
            }
        }
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn command_ids_are_unique() {
        let mut p1 = Project::new("api-backend", "/tmp/repo1", "main");
        p1.id = Some(101);
        let mut p2 = Project::new("web-frontend", "/tmp/repo2", "main");
        p2.id = Some(102);
        let dummy_projects = vec![p1, p2];

        let mut pkg1 = Package::new(
            101,
            "core-engine",
            "crates/core",
            "crates/core/Cargo.toml",
            None,
        );
        pkg1.id = Some(1);
        let dummy_packages = vec![pkg1];

        let commands = build_palette_commands(&dummy_projects, &dummy_packages);
        let mut seen = HashSet::new();
        for cmd in &commands {
            assert!(
                seen.insert(cmd.id.clone()),
                "Duplicate palette command id: {}",
                cmd.id
            );
        }
    }

    #[test]
    fn every_nav_tab_has_navigation_command() {
        let commands = build_palette_commands(&[], &[]);
        for tab in NavTab::ALL {
            let found = commands.iter().any(|cmd| match &cmd.action {
                PaletteAction::Navigate(t) => *t == tab,
                _ => false,
            });
            assert!(found, "Missing navigation command for tab: {:?}", tab);
        }
    }

    #[test]
    fn dynamic_project_switching_commands_generated() {
        let mut p = Project::new("core-engine", "/tmp/core", "main");
        p.id = Some(42);
        let dummy_projects = vec![p];

        let commands = build_palette_commands(&dummy_projects, &[]);
        let switch_cmd = commands
            .iter()
            .find(|c| c.id == "switch_repo_42")
            .expect("Expected switch_repo_42 command");

        assert_eq!(switch_cmd.title, "Switch to core-engine");
        assert_eq!(switch_cmd.category, "Repository");
        assert_eq!(switch_cmd.action, PaletteAction::SwitchProject(42));
    }
}
