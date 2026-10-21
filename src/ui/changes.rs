use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, IntoElement, RenderOnce, Window, div};

use crate::domain::commit::ChangeCategory;
use crate::services::conventional::CategoryMapper;
use crate::services::git_service::GitService;
use crate::ui::components::diff_view::DiffView;
use crate::ui::components::source_badge::SourceBadge;
use crate::ui::components::states::EmptyStateView;
use crate::ui::state::AppState;

#[derive(IntoElement)]
pub struct ChangesView {
    state: Entity<AppState>,
}

impl ChangesView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for ChangesView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let app_state = self.state.read(cx);

        let Some(project) = &app_state.active_project else {
            let state_open = self.state.clone();
            return v_flex().size_full().items_center().justify_center().child(
                EmptyStateView::new(
                    "No Project Selected",
                    "Open a Git repository to inspect commits and changes.",
                )
                .with_action("Open Repository", move |_, _, cx| {
                    state_open.update(cx, |s, cx| s.toggle_open_repo_dialog(true, cx));
                }),
            );
        };

        let filter = app_state.selected_category_filter;
        let selected_hash = app_state.selected_commit_hash.clone();
        let diff_preview = app_state.diff_preview.clone().unwrap_or_default();
        let diff_truncated = app_state.diff_truncated;

        let filtered_items: Vec<_> = app_state
            .change_items
            .iter()
            .enumerate()
            .filter(|(_, item)| match filter {
                Some(cat) => item.category == cat,
                None => true,
            })
            .collect();

        let state_clone_scan = self.state.clone();
        let repo_path = project.repository_path.clone();

        let filter_options: &[(Option<ChangeCategory>, &str, IconName)] = &[
            (None, "All", IconName::GitCommitHorizontal),
            (
                Some(ChangeCategory::Feature),
                "Features",
                IconName::Sparkles,
            ),
            (Some(ChangeCategory::Fix), "Fixes", IconName::Bug),
            (Some(ChangeCategory::Refactor), "Refactor", IconName::Code),
            (Some(ChangeCategory::Perf), "Perf", IconName::RefreshCw),
            (Some(ChangeCategory::Docs), "Docs", IconName::BookOpen),
            (Some(ChangeCategory::Chore), "Chore", IconName::Settings),
            (Some(ChangeCategory::Other), "Other", IconName::FileText),
        ];

        v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .p_6()
            .gap_4()
            .child(
                // Header & Action Bar
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
                                    .child("Git Change Intelligence (Changes)"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!(
                                        "{} commits detected | {} displayed (click a commit row to view diff)",
                                        app_state.commits.len(),
                                        filtered_items.len()
                                    )),
                            ),
                    )
                    .child(
                        Button::new("btn_rescan_git")
                            .icon(IconName::RefreshCw)
                            .label("Scan Git")
                            .primary()
                            .on_click(move |_, _, cx| {
                                let repo_path = repo_path.clone();
                                state_clone_scan.update(cx, |s, cx| {
                                    s.run_background(
                                        cx,
                                        "Scanning git commit history...",
                                        move || {
                                            let repo = git2::Repository::open(&repo_path)?;
                                            let mapper = CategoryMapper::default();
                                            let range =
                                                GitService::resolve_range(&repo, None, "HEAD")?;
                                            let commits =
                                                GitService::commits_in_range(&repo, &range)?;
                                            let items =
                                                GitService::generate_deterministic_change_items(
                                                    &repo, &range, &mapper,
                                                )?;
                                            Ok((range, commits, items))
                                        },
                                        |s, (range, commits, items), _| {
                                            s.commit_range = Some(range);
                                            s.commits = commits;
                                            s.change_items = items;
                                        },
                                    );
                                });
                            }),
                    ),
            )
            .child(
                // Category Filter Pills
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .children(filter_options.iter().map(|&(cat, label, icon)| {
                        let is_active = filter == cat;
                        let state = self.state.clone();
                        let btn = Button::new(format!("filter_{label}"))
                            .icon(icon)
                            .label(label)
                            .small();
                        let btn = if is_active {
                            btn.primary()
                        } else {
                            btn.secondary()
                        };
                        btn.on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.selected_category_filter = cat;
                                cx.notify();
                            });
                        })
                    })),
            )
            .child(
                // Split View: Items on left, Diff Preview on right
                h_flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_4()
                    .child(
                        // Commit item list card
                        v_flex()
                            .id("changes_commit_list_pane")
                            .flex_1()
                            .h_full()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .p_3()
                            .gap_2()
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.group_box)
                            .child(
                                // Sticky header: COMMITS + count
                                h_flex()
                                    .flex_none()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(theme.colors.muted_foreground)
                                            .child("COMMITS"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.muted_foreground)
                                            .child(format!("{} items", filtered_items.len())),
                                    ),
                            )
                            .child(
                                // Scrollable commits area
                                v_flex()
                                    .id("commit_items_scroll_area")
                                    .flex_1()
                                    .h_full()
                                    .min_h_0()
                                    .min_w_0()
                                    .overflow_y_scrollbar()
                                    .child(if filtered_items.is_empty() {
                                        div()
                                            .p_8()
                                            .items_center()
                                            .justify_center()
                                            .text_sm()
                                            .text_color(theme.colors.muted_foreground)
                                            .child("No commit items to display. Click 'Scan Git'.")
                                    } else {
                                        let mut list = v_flex().gap_2();
                                        for (idx, item) in filtered_items {
                                            let state_clone_toggle = self.state.clone();
                                            let state_clone_diff = self.state.clone();
                                            let full_hash = item.commit_hash.clone();
                                            let is_selected = full_hash.is_some()
                                                && full_hash == selected_hash;
                                            let hash_disp = item
                                                .commit_hash
                                                .as_deref()
                                                .map(|h| &h[..7.min(h.len())])
                                                .unwrap_or("no-hash");

                                            let inc_icon = if item.is_included {
                                                IconName::SquareCheck
                                            } else {
                                                IconName::Square
                                            };

                                            list = list.child(
                                                h_flex()
                                                    .p_2()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(if is_selected {
                                                        theme.colors.primary
                                                    } else {
                                                        theme.colors.border
                                                    })
                                                    .bg(if is_selected {
                                                        theme.colors.primary.opacity(0.10)
                                                    } else {
                                                        theme.colors.background
                                                    })
                                                    .justify_between()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .id(format!("toggle_inc_{idx}"))
                                                            .cursor_pointer()
                                                            .p_1()
                                                            .rounded_md()
                                                            .hover(|s| {
                                                                s.bg(theme
                                                                    .colors
                                                                    .accent
                                                                    .opacity(0.15))
                                                            })
                                                            .child(
                                                                Icon::new(inc_icon).text_color(
                                                                    if item.is_included {
                                                                        theme.colors.primary
                                                                    } else {
                                                                        theme
                                                                            .colors
                                                                            .muted_foreground
                                                                    },
                                                                ),
                                                            )
                                                            .on_click(move |_, _, cx| {
                                                                state_clone_toggle.update(
                                                                    cx,
                                                                    |s, cx| {
                                                                        if let Some(it) = s
                                                                            .change_items
                                                                            .get_mut(idx)
                                                                        {
                                                                            it.is_included =
                                                                                !it.is_included;
                                                                            cx.notify();
                                                                        }
                                                                    },
                                                                );
                                                            }),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .id(format!("select_commit_{idx}"))
                                                            .flex_1()
                                                            .min_w_0()
                                                            .justify_between()
                                                            .items_center()
                                                            .gap_2()
                                                            .cursor_pointer()
                                                            .on_click(move |_, _, cx| {
                                                                if let Some(hash) =
                                                                    full_hash.clone()
                                                                {
                                                                    state_clone_diff.update(
                                                                        cx,
                                                                        |s, cx| {
                                                                            if let Err(e) = s
                                                                                .select_commit_patch_sync(&hash)
                                                                            {
                                                                                s.set_error(Some(
                                                                                    e.to_string(),
                                                                                ));
                                                                            }
                                                                            cx.notify();
                                                                        },
                                                                    );
                                                                }
                                                            })
                                                            .child(
                                                                h_flex()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .gap_2()
                                                                    .items_center()
                                                                    .child(
                                                                        div()
                                                                            .flex_none()
                                                                            .text_xs()
                                                                            .font_family(
                                                                                "monospace",
                                                                            )
                                                                            .text_color(
                                                                                theme
                                                                                    .colors
                                                                                    .muted_foreground,
                                                                            )
                                                                            .child(
                                                                                hash_disp
                                                                                    .to_string(),
                                                                            ),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .flex_1()
                                                                            .min_w_0()
                                                                            .text_sm()
                                                                            .font_weight(
                                                                                gpui_kit::FontWeight::MEDIUM,
                                                                            )
                                                                            .text_color(
                                                                                theme
                                                                                    .colors
                                                                                    .foreground,
                                                                            )
                                                                            .child(
                                                                                item.title.clone(),
                                                                            ),
                                                                    ),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex_none()
                                                                    .child(SourceBadge::new(
                                                                        item.origin.clone(),
                                                                    )),
                                                            ),
                                                    ),
                                            );
                                        }
                                        list
                                    }),
                            ),
                    )
                    .child(
                        // Diff preview card
                        v_flex()
                            .id("changes_diff_preview_pane")
                            .flex_1()
                            .h_full()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .p_3()
                            .gap_2()
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.group_box)
                            .child(
                                // Sticky header: DIFF PREVIEW + commit hash
                                h_flex()
                                    .flex_none()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(theme.colors.muted_foreground)
                                            .child("DIFF PREVIEW"),
                                    )
                                    .when_some(selected_hash, |hdr, h| {
                                        hdr.child(
                                            div()
                                                .text_xs()
                                                .font_family("monospace")
                                                .text_color(theme.colors.primary)
                                                .child(h[..7.min(h.len())].to_string()),
                                        )
                                    }),
                            )
                            .child(
                                // Scrollable diff content area
                                v_flex()
                                    .id("diff_content_scroll_area")
                                    .flex_1()
                                    .h_full()
                                    .min_h_0()
                                    .min_w_0()
                                    .overflow_y_scrollbar()
                                    .child(
                                        div()
                                            .id("diff_horiz_scroll_container")
                                            .min_w_full()
                                            .overflow_x_scroll()
                                            .child(DiffView::new(diff_preview, diff_truncated)),
                                    ),
                            ),
                    ),
            )
    }
}
