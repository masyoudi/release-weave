use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, IntoElement, RenderOnce, Window, div};

use crate::services::readiness_service::ReadinessService;
use crate::ui::components::check_list::CheckListItem;
use crate::ui::components::states::EmptyStateView;
use crate::ui::state::AppState;

#[derive(IntoElement)]
pub struct ReadinessView {
    state: Entity<AppState>,
}

impl ReadinessView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for ReadinessView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let app_state = self.state.read(cx);

        let Some(project) = &app_state.active_project else {
            let state_open = self.state.clone();
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    EmptyStateView::new(
                        "No Project Selected",
                        "Open a Git repository to run release readiness checks.",
                    )
                    .with_action("Open Repository", move |_, _, cx| {
                        state_open.update(cx, |s, cx| s.toggle_open_repo_dialog(true, cx));
                    }),
                )
                .into_any_element();
        };

        let summary = &app_state.readiness_summary;
        let checks = &app_state.readiness_checks;
        let effective_settings = app_state.effective_settings();

        let (banner_title, banner_desc, banner_bg, banner_fg) = match summary {
            Some(s) if s.is_blocking => (
                "Release Blocked",
                format!(
                    "{} required checks have not passed ({} failed, {} not run).",
                    s.failed_count + s.not_run_count,
                    s.failed_count,
                    s.not_run_count
                ),
                theme.colors.danger.opacity(0.15),
                theme.colors.danger,
            ),
            Some(s) => (
                "Ready for Release",
                format!(
                    "All {} required checks passed successfully.",
                    s.passed_count
                ),
                theme.colors.success.opacity(0.15),
                theme.colors.success,
            ),
            None => (
                "Checks Not Run",
                "Click 'Run All Checks' to verify repository and dependencies.".to_string(),
                theme.colors.group_box,
                theme.colors.foreground,
            ),
        };

        let state_clone_run = self.state.clone();
        let state_clone_detect = self.state.clone();
        let state_clone_clear = self.state.clone();
        let state_clone_policy = self.state.clone();

        let repo_path = project.repository_path.clone();
        let settings = effective_settings.clone();
        let target_ver = if app_state.target_version.is_empty() {
            "0.1.0".to_string()
        } else {
            app_state.target_version.clone()
        };

        let detected = app_state
            .detected_stack
            .clone()
            .unwrap_or_else(|| ReadinessService::detect_project_stack(&repo_path));

        let cmds_count = settings.readiness_commands.len();
        let cmds_display = if cmds_count == 0 {
            "No command checks configured (Optional / Skipped)".to_string()
        } else {
            settings.readiness_commands.join(", ")
        };

        let is_required = settings.commands_required;

        v_flex()
            .id("readiness_scroll_view")
            .size_full()
            .overflow_y_scrollbar()
            .p_6()
            .pb_12()
            .gap_6()
            .child(
                // Header & Action
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child("Release Readiness Checks"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Verifying working tree, changelog, version consistency, and test suites"),
                            ),
                    )
                    .child(
                        Button::new("btn_run_checks")
                            .icon(IconName::Play)
                            .label("Run All Checks")
                            .primary()
                            .on_click(move |_, _, cx| {
                                let repo_path = repo_path.clone();
                                let target_ver = target_ver.clone();
                                let settings = settings.clone();
                                state_clone_run.update(cx, |s, cx| {
                                    s.run_background(
                                        cx,
                                        "Running release readiness checks...",
                                        move || {
                                            let repo = git2::Repository::open(&repo_path)?;
                                            let semver_target = semver::Version::parse(&target_ver)
                                                .unwrap_or_else(|_| semver::Version::new(0, 1, 0));
                                            let summary = ReadinessService::run_all(
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
                                });
                            }),
                    ),
            )
            .child(
                // Status Banner
                v_flex()
                    .w_full()
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(banner_bg)
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(banner_fg)
                            .child(banner_title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(banner_desc),
                    ),
            )
            .child(
                // Stack Detection & Readiness Commands Policy Card
                v_flex()
                    .w_full()
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .gap_3()
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(theme.colors.foreground)
                                            .child("Project Ecosystem:"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(theme.colors.primary.opacity(0.12))
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(theme.colors.primary)
                                            .child(format!(
                                                "{}{}",
                                                detected.language,
                                                detected
                                                    .manifest_file
                                                    .as_ref()
                                                    .map(|m| format!(" ({m})"))
                                                    .unwrap_or_default()
                                            )),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("btn_auto_detect_cmds")
                                            .icon(IconName::Sparkles)
                                            .label("Auto-Detect")
                                            .small()
                                            .secondary()
                                            .on_click({
                                                let detected_cmds = detected.suggested_commands.clone();
                                                move |_, _, cx| {
                                                    let cmds = detected_cmds.clone();
                                                    state_clone_detect.update(cx, |s, cx| {
                                                        let mut curr = s.effective_settings();
                                                        curr.readiness_commands = cmds;
                                                        _ = s.update_project_settings_sync(curr);
                                                        s.set_notice(Some("Readiness commands auto-detected from project.".to_string()));
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("btn_clear_cmds")
                                            .icon(IconName::Trash)
                                            .label("Clear (Skip)")
                                            .small()
                                            .secondary()
                                            .on_click(move |_, _, cx| {
                                                state_clone_clear.update(cx, |s, cx| {
                                                    let mut curr = s.effective_settings();
                                                    curr.readiness_commands.clear();
                                                    _ = s.update_project_settings_sync(curr);
                                                    s.set_notice(Some("Command checks disabled for this project.".to_string()));
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("btn_toggle_policy")
                                            .icon(if is_required {
                                                IconName::Lock
                                            } else {
                                                IconName::LockOpen
                                            })
                                            .label(if is_required {
                                                "Policy: Required"
                                            } else {
                                                "Policy: Optional"
                                            })
                                            .small()
                                            .secondary()
                                            .on_click(move |_, _, cx| {
                                                state_clone_policy.update(cx, |s, cx| {
                                                    let mut curr = s.effective_settings();
                                                    curr.commands_required = !curr.commands_required;
                                                    let notice = if curr.commands_required {
                                                        "Commands are now REQUIRED (will block release on failure)."
                                                    } else {
                                                        "Commands are now OPTIONAL (will only warn on failure)."
                                                    };
                                                    _ = s.update_project_settings_sync(curr);
                                                    s.set_notice(Some(notice.to_string()));
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(format!("Active commands: {cmds_display}")),
                    ),
            )
            .child(
                // Checks List
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(theme.colors.foreground)
                            .child("Readiness Checklist"),
                    )
                    .child(if checks.is_empty() {
                        div()
                            .p_8()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(theme.colors.muted_foreground)
                            .child("No checks have been run yet.")
                    } else {
                        let mut list = v_flex().gap_2();
                        for check in checks {
                            list = list.child(CheckListItem::new(check.clone()));
                        }
                        list
                    }),
            )
            .into_any_element()
    }
}
