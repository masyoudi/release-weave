use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, IntoElement, Render, RenderOnce, Subscription, Window, div, px,
};

use crate::services::git_service::GitService;
use crate::services::version_service::{BumpLevel, VersionService};
use crate::ui::components::states::EmptyStateView;
use crate::ui::state::AppState;

struct VersionInputHolder {
    input: Entity<InputState>,
    last_synced: String,
    _subscription: Subscription,
}

impl Render for VersionInputHolder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct VersioningView {
    state: Entity<AppState>,
}

impl VersioningView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for VersioningView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let app_state = self.state.read(cx);

        let Some(project) = &app_state.active_project else {
            let state_open = self.state.clone();
            return v_flex().size_full().items_center().justify_center().child(
                EmptyStateView::new(
                    "No Project Selected",
                    "Open a Git repository to inspect version analysis and SemVer recommendations.",
                )
                .with_action("Open Repository", move |_, _, cx| {
                    state_open.update(cx, |s, cx| s.toggle_open_repo_dialog(true, cx));
                }),
            ).into_any_element();
        };

        let current_ver = app_state
            .current_version
            .clone()
            .unwrap_or_else(|| "0.0.0".to_string());
        let rec = app_state.bump_recommendation.clone();
        let consistency = app_state.version_consistency.clone();

        let (bump_title, bump_reason, target_suggested) = match &rec {
            Some(b) => {
                let title = match b.level {
                    BumpLevel::Major => "Major Bump (Breaking Changes)",
                    BumpLevel::Minor => "Minor Bump (New Features)",
                    BumpLevel::Patch => "Patch Bump (Bug Fixes)",
                    BumpLevel::None => "No Bump (No new commits)",
                };
                let reason = b
                    .reasons
                    .first()
                    .map(|r| r.explanation.clone())
                    .unwrap_or_else(|| "Based on Conventional Commits".to_string());
                (
                    title.to_string(),
                    reason,
                    b.recommended_version.to_string(),
                )
            }
            None => (
                "Not Analyzed".to_string(),
                "Click 'Calculate Recommendation' to analyze Conventional commits and breaking changes.".to_string(),
                current_ver.clone(),
            ),
        };

        let target_display = if app_state.target_version.is_empty() {
            target_suggested
        } else {
            app_state.target_version.clone()
        };

        let state_clone_calc = self.state.clone();
        let repo_path = project.repository_path.clone();

        let initial_target = target_display.clone();
        let state_for_input = self.state.clone();
        let input_holder = window.use_state(cx, move |window, cx| {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("e.g. 1.2.0 or 1.2.0-rc.1")
                    .default_value(initial_target.clone())
            });
            let sub = cx.subscribe(
                &input,
                move |holder: &mut VersionInputHolder, input, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        let val = input.read(cx).value().to_string();
                        holder.last_synced = val.clone();
                        if let Ok(valid_ver) = VersionService::parse_manual_version(&val) {
                            state_for_input.update(cx, |s, cx| {
                                let ver_str = valid_ver.to_string();
                                if s.target_version != ver_str {
                                    s.target_version = ver_str;
                                    s.clear_error();
                                    cx.notify();
                                }
                            });
                        }
                    }
                },
            );
            VersionInputHolder {
                input,
                last_synced: initial_target,
                _subscription: sub,
            }
        });

        let target_for_sync = target_display.clone();
        let version_input_entity = input_holder.update(cx, |holder, cx| {
            if holder.last_synced != target_for_sync {
                holder.last_synced = target_for_sync.clone();
                let to_set = target_for_sync;
                holder.input.update(cx, |inp, cx| {
                    inp.set_value(to_set, window, cx);
                });
            }
            holder.input.clone()
        });

        let theme = cx.theme();

        v_flex()
            .id("versioning_scroll_view")
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
                                    .child("Versioning & SemVer Recommendation"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Automated SemVer analysis adhering to Semantic Versioning 2.0 specification"),
                            ),
                    )
                    .child(
                        Button::new("btn_calc_bump")
                            .icon(IconName::Sparkles)
                            .label("Calculate Recommendation")
                            .primary()
                            .on_click(move |_, _, cx| {
                                let repo_path = repo_path.clone();
                                state_clone_calc.update(cx, |s, cx| {
                                    s.run_background(
                                        cx,
                                        "Analyzing commits for version recommendation...",
                                        move || {
                                            let repo = git2::Repository::open(&repo_path)?;
                                            let cur = VersionService::read_current_version(&repo, &repo_path)?;
                                            let manifest = VersionService::read_manifest_version(&repo_path)?;
                                            let manifests = manifest.into_iter().collect::<Vec<_>>();
                                            let consistency = VersionService::check_consistency(
                                                Some(&cur),
                                                &manifests,
                                            );
                                            let range = GitService::resolve_range(&repo, None, "HEAD")?;
                                            let commits = GitService::commits_in_range(&repo, &range)?;
                                            let recommendation = VersionService::recommend_bump(
                                                &cur,
                                                &commits,
                                            );
                                            Ok((cur, consistency, recommendation))
                                        },
                                        |s, (cur, consistency, recommendation), _| {
                                            s.current_version = Some(cur.to_string());
                                            s.target_version = recommendation.recommended_version.to_string();
                                            s.version_consistency = Some(consistency);
                                            s.bump_recommendation = Some(recommendation);
                                            s.set_notice(Some("SemVer recommendation updated.".to_string()));
                                        },
                                    );
                                });
                            }),
                    ),
            )
            .child(
                // Current Version & Recommendation Cards
                h_flex()
                    .w_full()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .p_5()
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
                                    .text_3xl()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child(current_ver.clone()),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .p_5()
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
                                    .child("BUMP RECOMMENDATION"),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.primary)
                                    .child(bump_title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(bump_reason),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .p_5()
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
                                    .child("TARGET RELEASE VERSION"),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.success)
                                    .child(target_display.clone()),
                            ),
                    ),
            )
            .child(
                // Quick Bump & Custom Version Card
                v_flex()
                    .w_full()
                    .p_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .gap_4()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(theme.colors.foreground)
                            .child("Target Version Controls & Prerelease Helpers"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .items_center()
                            .child(
                                Button::new("btn_bump_major")
                                    .icon(IconName::Tag)
                                    .label("Bump Major")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let state = self.state.clone();
                                        let cur = current_ver.clone();
                                        move |_, _, cx| {
                                            if let Ok(v) = semver::Version::parse(&cur) {
                                                let next = semver::Version::new(v.major + 1, 0, 0);
                                                state.update(cx, |s, cx| {
                                                    s.target_version = next.to_string();
                                                    cx.notify();
                                                });
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_bump_minor")
                                    .icon(IconName::Tag)
                                    .label("Bump Minor")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let state = self.state.clone();
                                        let cur = current_ver.clone();
                                        move |_, _, cx| {
                                            if let Ok(v) = semver::Version::parse(&cur) {
                                                let next =
                                                    semver::Version::new(v.major, v.minor + 1, 0);
                                                state.update(cx, |s, cx| {
                                                    s.target_version = next.to_string();
                                                    cx.notify();
                                                });
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_bump_patch")
                                    .icon(IconName::Tag)
                                    .label("Bump Patch")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let state = self.state.clone();
                                        let cur = current_ver;
                                        move |_, _, cx| {
                                            if let Ok(v) = semver::Version::parse(&cur) {
                                                let next = semver::Version::new(
                                                    v.major,
                                                    v.minor,
                                                    v.patch + 1,
                                                );
                                                state.update(cx, |s, cx| {
                                                    s.target_version = next.to_string();
                                                    cx.notify();
                                                });
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_inc_rc")
                                    .icon(IconName::Tag)
                                    .label("Add -rc.1")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let state = self.state.clone();
                                        let cur = target_display.clone();
                                        move |_, _, cx| {
                                            if let Ok(cur_parsed) = semver::Version::parse(&cur)
                                                && let Ok(next) = VersionService::with_prerelease(
                                                    &cur_parsed,
                                                    "rc",
                                                )
                                            {
                                                state.update(cx, |s, cx| {
                                                    s.target_version = next.to_string();
                                                    cx.notify();
                                                });
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_inc_beta")
                                    .icon(IconName::Tag)
                                    .label("Add -beta.1")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let state = self.state.clone();
                                        let cur = target_display.clone();
                                        move |_, _, cx| {
                                            if let Ok(cur_parsed) = semver::Version::parse(&cur)
                                                && let Ok(next) = VersionService::with_prerelease(
                                                    &cur_parsed,
                                                    "beta",
                                                )
                                            {
                                                state.update(cx, |s, cx| {
                                                    s.target_version = next.to_string();
                                                    cx.notify();
                                                });
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_inc_pre")
                                    .icon(IconName::RefreshCw)
                                    .label("Increment Prerelease")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let state = self.state.clone();
                                        let cur = target_display;
                                        move |_, _, cx| {
                                            if let Ok(cur_parsed) = semver::Version::parse(&cur) {
                                                let prefix = cur_parsed
                                                    .pre
                                                    .as_str()
                                                    .rsplit_once('.')
                                                    .map(|(p, _)| p)
                                                    .filter(|p| !p.is_empty())
                                                    .unwrap_or("rc");
                                                if let Ok(next) = VersionService::with_prerelease(
                                                    &cur_parsed,
                                                    prefix,
                                                ) {
                                                    state.update(cx, |s, cx| {
                                                        s.target_version = next.to_string();
                                                        cx.notify();
                                                    });
                                                }
                                            }
                                        }
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Custom SemVer:"),
                            )
                            .child(div().w(px(240.0)).child(Input::new(&version_input_entity))),
                    ),
            )
            .when_some(consistency, |this, report| {
                let status_color = if report.is_consistent {
                    theme.colors.success
                } else {
                    theme.colors.warning
                };
                let status_icon = if report.is_consistent {
                    IconName::CircleCheck
                } else {
                    IconName::TriangleAlert
                };
                this.child(
                    v_flex()
                        .w_full()
                        .p_5()
                        .rounded_xl()
                        .border_1()
                        .border_color(theme.colors.border)
                        .bg(theme.colors.group_box)
                        .gap_2()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(status_icon).text_color(status_color))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(theme.colors.foreground)
                                        .child("Manifest & Git Tag Consistency"),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.colors.muted_foreground)
                                .child(report.message),
                        ),
                )
            })
            .when_some(rec, |this, b| {
                this.child(
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
                                .child("Detected Bump Reasons"),
                        )
                        .children(b.reasons.into_iter().take(6).map(|reason| {
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Icon::new(IconName::GitCommitHorizontal)
                                        .text_color(theme.colors.primary),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(theme.colors.foreground)
                                        .child(reason.summary),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.colors.muted_foreground)
                                        .child(format!("({})", reason.explanation)),
                                )
                        })),
                )
            })
            .into_any_element()
    }
}
