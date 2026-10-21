use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, IntoElement, RenderOnce, Window, div, px};

use crate::domain::release::ReleaseStatus;
use crate::services::report_service::ReleaseReport;
use crate::storage::repositories::{ReadinessRepo, ReleaseRepo};
use crate::ui::components::states::EmptyStateView;
use crate::ui::state::{AppState, NavTab};

#[derive(IntoElement)]
pub struct HistoryView {
    state: Entity<AppState>,
}

impl HistoryView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for HistoryView {
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
                        "Open a Git repository to view release history and audit reports.",
                    )
                    .with_action("Open Repository", move |_, _, cx| {
                        state_open.update(cx, |s, cx| s.toggle_open_repo_dialog(true, cx));
                    }),
                )
                .into_any_element();
        };

        let project_id = project.id.unwrap_or(1);
        let project_name = project.name.clone();
        let repo_path = project.repository_path.clone();
        let db_path = app_state.db_path.clone();
        let state_clone_load = self.state.clone();
        let releases = app_state.history_releases.clone();

        v_flex()
            .id("history_scroll_view")
            .size_full()
            .overflow_y_scrollbar()
            .p_6()
            .pb_12()
            .gap_6()
            .child(
                // Header
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
                                    .child("Release History & Audit Trail"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(
                                        "Final release records stored locally in SQLite database",
                                    ),
                            ),
                    )
                    .child(
                        Button::new("btn_refresh_history")
                            .icon(IconName::RefreshCw)
                            .label("Refresh History")
                            .primary()
                            .on_click({
                                let db_path = db_path.clone();
                                move |_, _, cx| {
                                    let db_path = db_path.clone();
                                    state_clone_load.update(cx, |s, cx| {
                                        s.run_background(
                                            cx,
                                            "Loading release history...",
                                            move || {
                                                let conn =
                                                    crate::storage::database::open_path(&db_path)?;
                                                ReleaseRepo::list_by_project(&conn, project_id)
                                            },
                                            |s, rels, _| {
                                                s.history_releases = rels;
                                            },
                                        );
                                    });
                                }
                            }),
                    ),
            )
            .child(
                // Release list
                v_flex().w_full().gap_3().child(if releases.is_empty() {
                    div()
                        .p_8()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(theme.colors.muted_foreground)
                        .child("No finalized releases found for this project.")
                } else {
                    let mut list = v_flex().gap_3();
                    for (idx, rel) in releases.into_iter().enumerate() {
                        let status_str = match rel.status {
                            ReleaseStatus::Final => "Final",
                            ReleaseStatus::Draft => "Draft",
                        };
                        let status_color = match rel.status {
                            ReleaseStatus::Final => theme.colors.success,
                            ReleaseStatus::Draft => theme.colors.warning,
                        };
                        let state_load_notes = self.state.clone();
                        let state_export_json = self.state.clone();
                        let state_export_md = self.state.clone();

                        let rel_for_notes = rel.clone();
                        let rel_for_json = rel.clone();
                        let rel_for_md = rel.clone();
                        let db_for_json = db_path.clone();
                        let db_for_md = db_path.clone();
                        let proj_for_json = project_name.clone();
                        let proj_for_md = project_name.clone();
                        let path_for_json = repo_path.clone();
                        let path_for_md = repo_path.clone();

                        list = list.child(
                            v_flex()
                                .p_4()
                                .rounded_xl()
                                .border_1()
                                .border_color(theme.colors.border)
                                .bg(theme.colors.group_box)
                                .gap_3()
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .flex_wrap()
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_base()
                                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                                        .text_color(theme.colors.foreground)
                                                        .child(format!("v{}", rel.version)),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py(px(2.0))
                                                        .rounded_md()
                                                        .bg(status_color.opacity(0.15))
                                                        .text_xs()
                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                        .text_color(status_color)
                                                        .child(status_str),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.colors.muted_foreground)
                                                .child(rel.created_at.to_rfc3339()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.colors.muted_foreground)
                                        .child(format!(
                                            "Commit range: {}..{}",
                                            rel.start_commit.as_deref().unwrap_or("root"),
                                            &rel.end_commit[..7.min(rel.end_commit.len())]
                                        )),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Button::new(format!("btn_hist_load_{idx}"))
                                                .icon(IconName::FileText)
                                                .label("Open in Editor")
                                                .small()
                                                .secondary()
                                                .on_click(move |_, _, cx| {
                                                    let notes = rel_for_notes.release_notes.clone();
                                                    let ver = rel_for_notes.version.clone();
                                                    state_load_notes.update(cx, |s, cx| {
                                                        s.release_notes = notes;
                                                        s.target_version = ver;
                                                        s.active_nav = NavTab::ReleaseNotes;
                                                        cx.notify();
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new(format!("btn_hist_json_{idx}"))
                                                .icon(IconName::Code)
                                                .label("Export Audit JSON")
                                                .small()
                                                .secondary()
                                                .on_click(move |_, _, cx| {
                                                    let rel = rel_for_json.clone();
                                                    let db = db_for_json.clone();
                                                    let proj_name = proj_for_json.clone();
                                                    let out_path = path_for_json.join(format!(
                                                        "RELEASE_REPORT_v{}.json",
                                                        rel.version
                                                    ));
                                                    state_export_json.update(cx, |s, cx| {
                                                        s.run_background(
                                                            cx,
                                                            "Exporting JSON audit report...",
                                                            move || {
                                                                let conn = crate::storage::database::open_path(&db)?;
                                                                let rid = rel.id.unwrap_or(0);
                                                                let items = ReleaseRepo::list_change_items(&conn, rid).unwrap_or_default();
                                                                let checks = ReadinessRepo::list_checks(&conn, rid).unwrap_or_default();
                                                                let report = ReleaseReport::build(proj_name, &rel, items, checks);
                                                                report.export_json(&out_path)
                                                            },
                                                            |s, saved_path, _| {
                                                                s.set_notice(Some(format!(
                                                                    "Audit JSON saved to {}",
                                                                    saved_path.display()
                                                                )));
                                                            },
                                                        );
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new(format!("btn_hist_md_{idx}"))
                                                .icon(IconName::Download)
                                                .label("Export Audit MD")
                                                .small()
                                                .secondary()
                                                .on_click(move |_, _, cx| {
                                                    let rel = rel_for_md.clone();
                                                    let db = db_for_md.clone();
                                                    let proj_name = proj_for_md.clone();
                                                    let out_path = path_for_md.join(format!(
                                                        "RELEASE_REPORT_v{}.md",
                                                        rel.version
                                                    ));
                                                    state_export_md.update(cx, |s, cx| {
                                                        s.run_background(
                                                            cx,
                                                            "Exporting Markdown audit report...",
                                                            move || {
                                                                let conn = crate::storage::database::open_path(&db)?;
                                                                let rid = rel.id.unwrap_or(0);
                                                                let items = ReleaseRepo::list_change_items(&conn, rid).unwrap_or_default();
                                                                let checks = ReadinessRepo::list_checks(&conn, rid).unwrap_or_default();
                                                                let report = ReleaseReport::build(proj_name, &rel, items, checks);
                                                                report.export_markdown(&out_path)
                                                            },
                                                            |s, saved_path, _| {
                                                                s.set_notice(Some(format!(
                                                                    "Audit Markdown saved to {}",
                                                                    saved_path.display()
                                                                )));
                                                            },
                                                        );
                                                    });
                                                }),
                                        ),
                                ),
                        );
                    }
                    list
                }),
            )
            .into_any_element()
    }
}
