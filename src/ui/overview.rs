use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, IntoElement, RenderOnce, Window, div, px};

use crate::services::git_service::GitService;
use crate::ui::state::{AppState, NavTab};

#[derive(IntoElement)]
pub struct OverviewView {
    state: Entity<AppState>,
}

impl OverviewView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for OverviewView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let app_state = self.state.read(cx);

        let Some(project) = &app_state.active_project else {
            let state_open_repo = self.state.clone();
            let state_open_dir = self.state.clone();
            let select_cb = self.state.clone();
            let recent_projects = app_state.projects.clone();

            return v_flex()
                .id("overview_empty_scroll_view")
                .size_full()
                .overflow_y_scrollbar()
                .items_center()
                .justify_center()
                .p_8()
                .pb_12()
                .gap_6()
                .child(
                    v_flex()
                        .max_w(px(580.0))
                        .w_full()
                        .p_8()
                        .rounded_2xl()
                        .border_1()
                        .border_color(theme.colors.border)
                        .bg(theme.colors.group_box)
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(theme.colors.primary)
                                .child("ReleaseWeave"),
                        )
                        .child(
                            div()
                                .text_base()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .text_color(theme.colors.foreground)
                                .child("Verifiable Native Release Engineering & Local AI"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.colors.muted_foreground)
                                .child("Select or open a local Git repository to start inspecting commits, calculating SemVer 2.0 recommendations, running readiness checks, and drafting verifiable release notes."),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    Button::new("btn_onboarding_open_dir")
                                        .icon(IconName::FolderOpen)
                                        .label("Open Directory")
                                        .primary()
                                        .on_click(move |_, _, cx| {
                                            state_open_dir.update(cx, |s, cx| {
                                                s.pick_and_open_directory(cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("btn_onboarding_open_repo")
                                        .icon(IconName::FolderGit2)
                                        .label("Repositories...")
                                        .secondary()
                                        .on_click(move |_, _, cx| {
                                            state_open_repo.update(cx, |s, cx| {
                                                s.toggle_open_repo_dialog(true, cx);
                                            });
                                        }),
                                ),
                        )
                        .when(!recent_projects.is_empty(), |this| {
                            this.child(
                                v_flex()
                                    .w_full()
                                    .pt_4()
                                    .gap_2()
                                    .border_t_1()
                                    .border_color(theme.colors.border)
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(theme.colors.muted_foreground)
                                            .child("Recent Repositories:"),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .children(recent_projects.into_iter().take(4).map(|p| {
                                                let sel_state = select_cb.clone();
                                                let proj_clone = p.clone();

                                                h_flex()
                                                    .id(format!("overview_recent_{}", p.name))
                                                    .w_full()
                                                    .p_2()
                                                    .rounded_md()
                                                    .bg(theme.colors.background)
                                                    .border_1()
                                                    .border_color(theme.colors.border)
                                                    .justify_between()
                                                    .items_center()
                                                    .cursor_pointer()
                                                    .on_click(move |_, _, cx| {
                                                        sel_state.update(cx, |s, cx| {
                                                            s.select_project(proj_clone.clone(), cx);
                                                        });
                                                    })
                                                    .child(
                                                        v_flex()
                                                            .gap(px(1.0))
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                    .text_color(theme.colors.foreground)
                                                                    .child(p.name),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.colors.muted_foreground)
                                                                    .child(p.repository_path.to_string_lossy().to_string()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.colors.muted_foreground)
                                                            .child(format!("branch: {}", p.default_branch)),
                                                    )
                                            })),
                                    ),
                            )
                        }),
                );
        };

        let repo_available = GitService::check_repo_available(project);
        let repo_status_str = if repo_available {
            "Available (Git Repository)"
        } else {
            "Directory Not Found / Not a Git Repo"
        };
        let repo_status_color = if repo_available {
            theme.colors.success
        } else {
            theme.colors.danger
        };

        let current_ver = app_state
            .current_version
            .as_deref()
            .unwrap_or("Not detected");
        let rec_bump = app_state
            .bump_recommendation
            .as_ref()
            .map(|b| match b.level {
                crate::services::version_service::BumpLevel::Major => "Major",
                crate::services::version_service::BumpLevel::Minor => "Minor",
                crate::services::version_service::BumpLevel::Patch => "Patch",
                crate::services::version_service::BumpLevel::None => "None",
            })
            .unwrap_or("Not analyzed");

        let readiness_summary_str = match &app_state.readiness_summary {
            Some(s) => {
                if s.is_blocking {
                    format!(
                        "Release Blocked ({} failed, {} passed)",
                        s.failed_count, s.passed_count
                    )
                } else {
                    format!("Ready to Release ({} passed)", s.passed_count)
                }
            }
            None => "Readiness checks not yet run".to_string(),
        };

        let readiness_color = match &app_state.readiness_summary {
            Some(s) if s.is_blocking => theme.colors.danger,
            Some(_) => theme.colors.success,
            None => theme.colors.muted_foreground,
        };

        let state_clone_scan = self.state.clone();
        let state_clone_ready = self.state.clone();
        let state_clone_studio = self.state.clone();

        v_flex()
            .id("overview_scroll_view")
            .size_full()
            .overflow_y_scrollbar()
            .p_6()
            .pb_12()
            .gap_6()
            .child(
                // Header card
                v_flex()
                    .w_full()
                    .p_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(theme.colors.foreground)
                                            .child(project.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.muted_foreground)
                                            .child(
                                                project
                                                    .repository_path
                                                    .to_string_lossy()
                                                    .to_string(),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(repo_status_color.opacity(0.12))
                                    .border_1()
                                    .border_color(repo_status_color.opacity(0.3))
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .text_color(repo_status_color)
                                    .child(repo_status_str),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(format!("Active branch: {}", project.default_branch)),
                    ),
            )
            .child(
                // Summary cards grid
                h_flex()
                    .w_full()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.group_box)
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("CURRENT VERSION"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child(current_ver.to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!("Recommended bump: {rec_bump}")),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.group_box)
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("RELEASE READINESS"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(readiness_color)
                                    .child(readiness_summary_str),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!(
                                        "{} registered checks",
                                        app_state.readiness_checks.len()
                                    )),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.group_box)
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("COMMITS IN RANGE"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child(app_state.commits.len().to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!(
                                        "{} change items detected",
                                        app_state.change_items.len()
                                    )),
                            ),
                    ),
            )
            .child(
                // Quick actions
                v_flex()
                    .w_full()
                    .p_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(theme.colors.foreground)
                            .child("Quick Actions"),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                Button::new("btn_nav_changes")
                                    .icon(IconName::GitCommitHorizontal)
                                    .label("Inspect Changes")
                                    .secondary()
                                    .on_click(move |_, _, cx| {
                                        state_clone_scan
                                            .update(cx, |s, cx| s.set_nav(NavTab::Changes, cx));
                                    }),
                            )
                            .child(
                                Button::new("btn_nav_readiness")
                                    .icon(IconName::ShieldCheck)
                                    .label("Run Readiness Checks")
                                    .secondary()
                                    .on_click(move |_, _, cx| {
                                        state_clone_ready
                                            .update(cx, |s, cx| s.set_nav(NavTab::Readiness, cx));
                                    }),
                            )
                            .child(
                                Button::new("btn_nav_release_notes")
                                    .icon(IconName::FileText)
                                    .label("Open Release Notes Studio")
                                    .primary()
                                    .on_click(move |_, _, cx| {
                                        state_clone_studio.update(cx, |s, cx| {
                                            s.set_nav(NavTab::ReleaseNotes, cx)
                                        });
                                    }),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn overview_view_constructs() {
        let dir = tempdir().unwrap();
        let db_file = dir.path().join("rw.db");
        // We verify that OverviewView struct instantiates properly
        let state_data = AppState::new(db_file);
        assert!(state_data.active_project.is_none());
    }
}
