use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClipboardItem, Entity, IntoElement, RenderOnce, Window, div};

use crate::ai::ollama::OllamaProvider;
use crate::services::release_service::{DraftOptions, ReleaseService};
use crate::ui::components::markdown_editor::MarkdownEditor;
use crate::ui::components::markdown_preview::MarkdownPreview;
use crate::ui::components::states::EmptyStateView;
use crate::ui::state::AppState;

#[derive(IntoElement)]
pub struct ReleaseNotesView {
    state: Entity<AppState>,
}

impl ReleaseNotesView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for ReleaseNotesView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let app_state = self.state.read(cx);

        let Some(project) = &app_state.active_project else {
            let state_open = self.state.clone();
            return v_flex().size_full().items_center().justify_center().child(
                EmptyStateView::new(
                    "No Project Selected",
                    "Open a Git repository to compose and preview release notes.",
                )
                .with_action("Open Repository", move |_, _, cx| {
                    state_open.update(cx, |s, cx| s.toggle_open_repo_dialog(true, cx));
                }),
            );
        };

        let target_ver = if app_state.target_version.is_empty() {
            "0.1.0".to_string()
        } else {
            app_state.target_version.clone()
        };

        let prev_ver = app_state.current_version.clone();
        let commits = app_state.commits.clone();
        let items = app_state.change_items.clone();
        let template_id = app_state.selected_template_id.clone();
        let template_str = app_state.active_template_str();
        let template_label = if template_id == "simple-markdown" {
            "Simple Markdown"
        } else {
            "Keep a Changelog"
        };

        let ollama_url = app_state
            .global_settings
            .ollama_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let ollama_model = app_state
            .global_settings
            .model
            .clone()
            .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
        let context_limit = app_state.global_settings.context_limit.unwrap_or(12000);

        let state_clone_det = self.state.clone();
        let state_clone_ai = self.state.clone();
        let state_clone_export = self.state.clone();
        let state_clone_copy = self.state.clone();
        let state_clone_edit = self.state.clone();
        let state_clone_tpl = self.state.clone();
        let state_clone_finalize_btn = self.state.clone();

        let editor_text = app_state.release_notes.clone();
        let notes_text = if app_state.release_notes.is_empty() {
            "# Release Notes\n\n*(No draft yet. Type in the editor or click 'Deterministic' / 'Generate AI' to start)*"
                .to_string()
        } else {
            app_state.release_notes.clone()
        };

        let project_path = project.repository_path.clone();

        let is_release_final = app_state
            .active_release
            .as_ref()
            .map(|r| r.status == crate::domain::release::ReleaseStatus::Final)
            .unwrap_or(false);
        let allow_git_tag = app_state.effective_settings().allow_git_tag;

        let remote_res = git2::Repository::open(&project.repository_path)
            .ok()
            .and_then(|r| {
                crate::services::remote_service::RemoteService::detect_remote(&r, None).ok()
            });
        let has_remote = remote_res.is_some();
        let remote_host = remote_res
            .as_ref()
            .map(|r| r.host.clone())
            .unwrap_or_default();
        let remote_provider_name = remote_res
            .as_ref()
            .map(|r| r.provider.display_name().to_string())
            .unwrap_or_else(|| "Remote".to_string());
        let has_remote_token = !remote_host.is_empty()
            && crate::security::secrets::SecretService::get_token(&remote_host)
                .ok()
                .flatten()
                .is_some();

        let root = v_flex()
            .size_full()
            .min_h_0()
            .p_6()
            .gap_4()
            .child(
                // Toolbar
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child("Release Notes Studio"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!(
                                        "Target: v{} | Template: {}",
                                        target_ver, template_label
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new("btn_switch_template")
                                    .icon(IconName::FileText)
                                    .label(template_label)
                                    .small()
                                    .secondary()
                                    .on_click(move |_, _, cx| {
                                        state_clone_tpl.update(cx, |s, cx| {
                                            s.selected_template_id =
                                                if s.selected_template_id == "keep-a-changelog" {
                                                    "simple-markdown".to_string()
                                                } else {
                                                    "keep-a-changelog".to_string()
                                                };
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new("btn_gen_det")
                                    .icon(IconName::Code)
                                    .label("Deterministic")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let items = items.clone();
                                        let target = target_ver.clone();
                                        let prev = prev_ver.clone();
                                        move |_, _, cx| {
                                            let items = items.clone();
                                            let target = target.clone();
                                            let prev = prev.clone();
                                            state_clone_det.update(cx, |s, cx| {
                                                let tpl = s.active_template_str();
                                                s.run_background(
                                                    cx,
                                                    "Composing deterministic draft...",
                                                    move || {
                                                        ReleaseService::generate_deterministic_draft(
                                                            &target,
                                                            &items,
                                                            tpl,
                                                            prev.as_deref(),
                                                        )
                                                    },
                                                    |s, draft, _| {
                                                        s.release_notes = draft;
                                                    },
                                                );
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_gen_ai")
                                    .icon(IconName::Sparkles)
                                    .label("Generate AI (Ollama)")
                                    .small()
                                    .primary()
                                    .on_click({
                                        let commits = commits.clone();
                                        let target = target_ver.clone();
                                        let prev = prev_ver.clone();
                                        let url = ollama_url;
                                        let model = ollama_model;
                                        move |_, _, cx| {
                                            let commits = commits.clone();
                                            let target = target.clone();
                                            let prev = prev.clone();
                                            let url = url.clone();
                                            let model = model.clone();
                                            state_clone_ai.update(cx, |s, cx| {
                                                let tpl = s.active_template_str();
                                                s.run_background(
                                                    cx,
                                                    "Invoking local Ollama model...",
                                                    move || {
                                                        let provider =
                                                            OllamaProvider::new(&url, &model);
                                                        let opts = DraftOptions {
                                                            version: &target,
                                                            template_str: tpl,
                                                            previous_version: prev.as_deref(),
                                                            context_limit_chars: context_limit,
                                                        };
                                                        ReleaseService::generate_ai_draft(
                                                            &provider,
                                                            &commits,
                                                            &opts,
                                                        )
                                                    },
                                                    |s, res, _| {
                                                        s.release_notes = res.release_notes;
                                                        s.change_items = res.items;
                                                        s.context_report = Some(res.context_report);
                                                    },
                                                );
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_copy_md")
                                    .icon(IconName::Copy)
                                    .label("Copy")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let notes = notes_text.clone();
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                notes.clone(),
                                            ));
                                            state_clone_copy.update(cx, |s, cx| {
                                                s.set_notice(Some(
                                                    "Release notes copied to clipboard.".to_string(),
                                                ));
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("btn_export_md")
                                    .icon(IconName::Download)
                                    .label("Export Markdown")
                                    .small()
                                    .secondary()
                                    .on_click({
                                        let notes = notes_text.clone();
                                        let path = project_path.join("RELEASE_NOTES.md");
                                        move |_, _, cx| {
                                            let notes = notes.clone();
                                            let path = path.clone();
                                            state_clone_export.update(cx, |s, cx| {
                                                s.run_background(
                                                    cx,
                                                    "Exporting RELEASE_NOTES.md...",
                                                    move || {
                                                        ReleaseService::export_markdown(&notes, &path)
                                                    },
                                                    |s, out_path, _| {
                                                        s.set_error(None);
                                                        s.set_notice(Some(format!(
                                                            "Exported to {}",
                                                            out_path.display()
                                                        )));
                                                    },
                                                );
                                            });
                                        }
                                    }),
                            )
                            .child({
                                let state_clone_apply = self.state.clone();
                                Button::new("btn_apply_files")
                                    .icon(IconName::FileText)
                                    .label("Apply to Files")
                                    .small()
                                    .primary()
                                    .tooltip("Preview and apply target version and changelog to project files")
                                    .on_click(move |_, _, cx| {
                                        state_clone_apply.update(cx, |s, cx| {
                                            s.plan_file_changes_sync(cx);
                                        });
                                    })
                            })
                            .child(
                                Button::new("btn_finalize")
                                    .icon(IconName::CheckCheck)
                                    .label("Finalize Release")
                                    .small()
                                    .success()
                                    .on_click(move |_, _, cx| {
                                        state_clone_finalize_btn.update(cx, |s, cx| {
                                            s.show_finalize_confirm = true;
                                            cx.notify();
                                        });
                                    }),
                            )
                            .when(allow_git_tag && is_release_final, |row| {
                                let state_clone_tag = self.state.clone();
                                row.child(
                                    Button::new("btn_create_git_tag")
                                        .icon(IconName::Tag)
                                        .label("Create Tag")
                                        .small()
                                        .primary()
                                        .tooltip("Create local annotated Git tag on HEAD")
                                        .on_click(move |_, _, cx| {
                                            state_clone_tag.update(cx, |s, cx| {
                                                s.show_tag_confirm = true;
                                                cx.notify();
                                            });
                                        }),
                                )
                            })
                            .when(is_release_final && has_remote, |row| {
                                let state_clone_pub = self.state.clone();
                                row.child(
                                    Button::new("btn_publish_draft")
                                        .icon(IconName::Upload)
                                        .label(format!("Publish Draft ({})", remote_provider_name))
                                        .small()
                                        .primary()
                                        .disabled(!has_remote_token)
                                        .tooltip(if has_remote_token {
                                            "Publish this finalized release as a draft to remote provider"
                                        } else {
                                            "Configure access token in Settings first"
                                        })
                                        .on_click(move |_, _, cx| {
                                            state_clone_pub.update(cx, |s, cx| {
                                                s.publish_draft_release_sync(cx);
                                            });
                                        }),
                                )
                            }),
                    ),
            )
            .child(
                // Editor & Preview Split
                h_flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .gap_4()
                    .child(
                        v_flex()
                            .id("release_notes_editor_pane")
                            .flex_1()
                            .h_full()
                            .min_h_0()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("EDITOR (MARKDOWN BUFFER)"),
                            )
                            .child(
                                MarkdownEditor::new(editor_text).on_change(move |updated, cx| {
                                    state_clone_edit.update(cx, |s, cx| {
                                        if s.release_notes != updated {
                                            s.release_notes = updated;
                                            cx.notify();
                                        }
                                    });
                                }),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("release_notes_preview_pane")
                            .flex_1()
                            .h_full()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("LIVE PREVIEW"),
                            )
                            .child(MarkdownPreview::new(notes_text)),
                    ),
            );

        let _ = template_str;
        root
    }
}
