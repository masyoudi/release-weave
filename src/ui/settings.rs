use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, IntoElement, Render, RenderOnce, Window, div};

use crate::domain::project::ProjectSettings;
use crate::errors::AppError;
use crate::security::secrets::SecretService;
use crate::services::publish::github::GitHubClient;
use crate::services::publish::gitlab::GitLabClient;
use crate::ui::state::AppState;

struct SettingsInputHolder {
    url_input: Entity<InputState>,
    model_input: Entity<InputState>,
    context_input: Entity<InputState>,
    commands_input: Entity<InputState>,
    commands_required: bool,
    allow_git_tag: bool,
    gh_token_input: Entity<InputState>,
    gl_token_input: Entity<InputState>,
    gh_configured: bool,
    gl_configured: bool,
}

impl Render for SettingsInputHolder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct SettingsView {
    state: Entity<AppState>,
}

impl SettingsView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for SettingsView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let app_state = self.state.read(cx);
        let current_settings = app_state.effective_settings();
        let detected_opt = app_state.detected_stack.clone().or_else(|| {
            app_state.active_project.as_ref().map(|p| {
                crate::services::readiness_service::ReadinessService::detect_project_stack(
                    &p.repository_path,
                )
            })
        });

        let initial_url = current_settings
            .ollama_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let initial_model = current_settings
            .model
            .clone()
            .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
        let initial_context = current_settings.context_limit.unwrap_or(12_000).to_string();
        let initial_commands = current_settings.readiness_commands.join(", ");
        let initial_required = current_settings.commands_required;
        let initial_allow_git_tag = current_settings.allow_git_tag;

        let holder = window.use_state(cx, move |window, cx| {
            let url_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("http://localhost:11434")
                    .default_value(initial_url)
            });
            let model_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("qwen2.5-coder:7b")
                    .default_value(initial_model)
            });
            let context_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("12000")
                    .default_value(initial_context)
            });
            let commands_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Optional (e.g. cargo test, npm test, pytest)")
                    .default_value(initial_commands)
            });
            let gh_token_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("ghp_... or github_pat_..."));
            let gl_token_input = cx.new(|cx| InputState::new(window, cx).placeholder("glpat-..."));
            let gh_configured = SecretService::get_token("github.com")
                .ok()
                .flatten()
                .is_some();
            let gl_configured = SecretService::get_token("gitlab.com")
                .ok()
                .flatten()
                .is_some();

            SettingsInputHolder {
                url_input,
                model_input,
                context_input,
                commands_input,
                commands_required: initial_required,
                allow_git_tag: initial_allow_git_tag,
                gh_token_input,
                gl_token_input,
                gh_configured,
                gl_configured,
            }
        });

        let theme = cx.theme();
        let is_dark = theme.is_dark();
        let theme_str = if is_dark { "Dark Mode" } else { "Light Mode" };

        let url_input = holder.read(cx).url_input.clone();
        let model_input = holder.read(cx).model_input.clone();
        let context_input = holder.read(cx).context_input.clone();
        let commands_input = holder.read(cx).commands_input.clone();
        let commands_required = holder.read(cx).commands_required;
        let allow_git_tag = holder.read(cx).allow_git_tag;
        let gh_token_input = holder.read(cx).gh_token_input.clone();
        let gl_token_input = holder.read(cx).gl_token_input.clone();
        let gh_configured = holder.read(cx).gh_configured;
        let gl_configured = holder.read(cx).gl_configured;
        let holder_for_remote = holder.clone();

        let state_clone_theme = self.state.clone();
        let state_clone_save = self.state.clone();
        let state_clone_reset = self.state.clone();

        let url_for_save = url_input.clone();
        let model_for_save = model_input.clone();
        let context_for_save = context_input.clone();
        let commands_for_save = commands_input.clone();

        let url_for_reset = url_input.clone();
        let model_for_reset = model_input.clone();
        let context_for_reset = context_input.clone();
        let commands_for_reset = commands_input.clone();

        let commands_for_detect = commands_input.clone();
        let commands_for_clear = commands_input.clone();
        let holder_for_policy = holder.clone();
        let holder_for_tag = holder.clone();
        let holder_for_reset = holder.clone();

        v_flex()
            .id("settings_scroll_view")
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
                                    .child("Application & Project Settings"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Configure Ollama AI provider, default templates, and readiness commands"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("btn_reset_settings")
                                    .icon(IconName::Undo)
                                    .label("Reset Defaults")
                                    .secondary()
                                    .on_click(move |_, window, cx| {
                                        let defaults = AppState::default_global_settings();
                                        let cmds = defaults.readiness_commands.join(", ");
                                        let def_url = defaults
                                            .ollama_url
                                            .clone()
                                            .unwrap_or_else(|| "http://localhost:11434".to_string());
                                        let def_model = defaults
                                            .model
                                            .clone()
                                            .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
                                        let def_ctx = defaults
                                            .context_limit
                                            .unwrap_or(12_000)
                                            .to_string();
                                        url_for_reset.update(cx, |inp, cx| {
                                            inp.set_value(def_url, window, cx);
                                        });
                                        model_for_reset.update(cx, |inp, cx| {
                                            inp.set_value(def_model, window, cx);
                                        });
                                        context_for_reset.update(cx, |inp, cx| {
                                            inp.set_value(def_ctx, window, cx);
                                        });
                                        commands_for_reset.update(cx, |inp, cx| {
                                            inp.set_value(cmds, window, cx);
                                        });
                                        holder_for_reset.update(cx, |h, _| {
                                            h.commands_required = false;
                                            h.allow_git_tag = false;
                                        });
                                        state_clone_reset.update(cx, |s, cx| {
                                            if let Err(e) = s.save_global_settings_sync(defaults.clone()) {
                                                s.set_error(Some(e.to_string()));
                                            } else {
                                                _ = s.update_project_settings_sync(defaults);
                                                s.set_notice(Some("Settings restored to defaults.".to_string()));
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new("btn_save_settings")
                                    .icon(IconName::SaveCheck)
                                    .label("Save Settings")
                                    .primary()
                                    .on_click(move |_, _, cx| {
                                        let url_val = url_for_save.read(cx).value().trim().to_string();
                                        let model_val = model_for_save.read(cx).value().trim().to_string();
                                        let ctx_val = context_for_save
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .parse::<usize>()
                                            .unwrap_or(12_000);
                                        let cmds_val: Vec<String> = commands_for_save
                                            .read(cx)
                                            .value()
                                            .split(',')
                                            .map(|c| c.trim().to_string())
                                            .filter(|c| !c.is_empty())
                                            .collect();

                                        state_clone_save.update(cx, |s, cx| {
                                            let new_settings = ProjectSettings {
                                                ollama_url: Some(if url_val.is_empty() {
                                                    "http://localhost:11434".to_string()
                                                } else {
                                                    url_val
                                                }),
                                                model: Some(if model_val.is_empty() {
                                                    "qwen2.5-coder:7b".to_string()
                                                } else {
                                                    model_val
                                                }),
                                                context_limit: Some(ctx_val.max(1000)),
                                                readiness_commands: cmds_val,
                                                language: Some("en".to_string()),
                                                commands_required,
                                                allow_git_tag,
                                                package_tag_pattern: current_settings
                                                    .package_tag_pattern,
                                            };
                                            if let Err(e) = s.save_global_settings_sync(new_settings.clone()) {
                                                s.set_error(Some(e.to_string()));
                                            } else {
                                                _ = s.update_project_settings_sync(new_settings);
                                                s.clear_error();
                                                s.set_notice(Some("Settings saved to database.".to_string()));
                                            }
                                            cx.notify();
                                        });
                                    }),
                            ),
                    ),
            )
            .child(
                // Theme Toggle Card (T-102)
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
                            .child("Appearance & Theme"),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!(
                                        "Active palette: {} ({})",
                                        theme.theme_name(),
                                        theme_str
                                    )),
                            )
                            .child(
                                Button::new("btn_toggle_theme")
                                    .icon(if is_dark {
                                        IconName::Sun
                                    } else {
                                        IconName::Moon
                                    })
                                    .label(if is_dark {
                                        "Switch to Light"
                                    } else {
                                        "Switch to Dark"
                                    })
                                    .secondary()
                                    .on_click(move |_, _, cx| {
                                        let mode = if cx.theme().is_dark() {
                                            ThemeMode::Light
                                        } else {
                                            ThemeMode::Dark
                                        };
                                        Theme::change(mode, None, cx);
                                        state_clone_theme.update(cx, |s, cx| {
                                            s.toggle_theme(cx);
                                        });
                                    }),
                            ),
                    ),
            )
            .child(
                // AI Local Provider Card
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
                            .child("Local Ollama AI Configuration"),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .text_color(theme.colors.muted_foreground)
                                            .child("Endpoint URL"),
                                    )
                                    .child(Input::new(&url_input).cleanable(true)),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_4()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                    .text_color(theme.colors.muted_foreground)
                                                    .child("Default Model"),
                                            )
                                            .child(Input::new(&model_input).cleanable(true)),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                    .text_color(theme.colors.muted_foreground)
                                                    .child("Context Limit (characters)"),
                                            )
                                            .child(Input::new(&context_input).cleanable(true)),
                                    ),
                            ),
                    ),
            )
            .child(
                // Readiness Commands Card
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
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(theme.colors.foreground)
                                            .child("Release Readiness Commands"),
                                    )
                                    .child(if let Some(stack) = &detected_opt {
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
                                                stack.language,
                                                stack
                                                    .manifest_file
                                                    .as_ref()
                                                    .map(|m| format!(" ({m})"))
                                                    .unwrap_or_default()
                                            ))
                                    } else {
                                        div()
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("btn_detect_settings")
                                            .icon(IconName::Sparkles)
                                            .label("Auto-Detect")
                                            .small()
                                            .secondary()
                                            .on_click({
                                                let suggested = detected_opt
                                                    .as_ref()
                                                    .map(|s| s.suggested_commands.join(", "))
                                                    .unwrap_or_default();
                                                move |_, window, cx| {
                                                    commands_for_detect.update(cx, |inp, cx| {
                                                        inp.set_value(suggested.clone(), window, cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("btn_clear_settings_cmds")
                                            .icon(IconName::Trash)
                                            .label("Clear (Skip)")
                                            .small()
                                            .secondary()
                                            .on_click(move |_, window, cx| {
                                                commands_for_clear.update(cx, |inp, cx| {
                                                    inp.set_value(String::new(), window, cx);
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("btn_toggle_policy_settings")
                                            .icon(if commands_required {
                                                IconName::Lock
                                            } else {
                                                IconName::LockOpen
                                            })
                                            .label(if commands_required {
                                                "Policy: Required"
                                            } else {
                                                "Policy: Optional"
                                            })
                                            .small()
                                            .secondary()
                                            .on_click(move |_, _, cx| {
                                                holder_for_policy.update(cx, |h, cx| {
                                                    h.commands_required = !h.commands_required;
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
                            .child("Comma-separated shell commands executed in the repository root. When Optional, command failures produce warnings without blocking release."),
                    )
                    .child(Input::new(&commands_input).cleanable(true)),
            )
            .child(
                // Git Version Control Options Card
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
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(theme.colors.foreground)
                                            .child("Git Version Control"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(if allow_git_tag {
                                                theme.colors.primary.opacity(0.12)
                                            } else {
                                                theme.colors.muted.opacity(0.4)
                                            })
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(if allow_git_tag {
                                                theme.colors.primary
                                            } else {
                                                theme.colors.muted_foreground
                                            })
                                            .child(if allow_git_tag {
                                                "Tagging Enabled"
                                            } else {
                                                "Tagging Disabled (Default)"
                                            }),
                                    ),
                            )
                            .child(
                                Button::new("btn_toggle_git_tag_settings")
                                    .icon(IconName::Tag)
                                    .label(if allow_git_tag {
                                        "Tagging: Allowed"
                                    } else {
                                        "Tagging: Disabled"
                                    })
                                    .small()
                                    .secondary()
                                    .on_click(move |_, _, cx| {
                                        holder_for_tag.update(cx, |h, cx| {
                                            h.allow_git_tag = !h.allow_git_tag;
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child("Allow creating local annotated Git tags (e.g. v1.0.0) on HEAD during release finalization. Tags are strictly created locally and are never pushed to remote repositories automatically."),
                    ),
            )
            .child(
                // Remote Release Credentials Card
                v_flex()
                    .w_full()
                    .p_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child("Remote Release Credentials"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Personal access tokens are stored securely in your OS Keychain and used to publish draft releases. Tokens are never saved to SQLite, stored in git, or logged."),
                            ),
                    )
                    // GitHub section
                    .child(
                        v_flex()
                            .gap_2()
                            .p_3p5()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme.colors.border.opacity(0.7))
                            .bg(theme.colors.muted.opacity(0.15))
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
                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .text_color(theme.colors.foreground)
                                                    .child("GitHub (github.com)"),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(if gh_configured {
                                                        theme.colors.primary.opacity(0.12)
                                                    } else {
                                                        theme.colors.muted.opacity(0.4)
                                                    })
                                                    .text_xs()
                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .text_color(if gh_configured {
                                                        theme.colors.primary
                                                    } else {
                                                        theme.colors.muted_foreground
                                                    })
                                                    .child(if gh_configured {
                                                        "Configured in Keychain"
                                                    } else {
                                                        "Not Configured"
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child({
                                                let state_test = self.state.clone();
                                                Button::new("btn_test_gh_token")
                                                    .icon(IconName::Plug)
                                                    .label("Test Connection")
                                                    .small()
                                                    .secondary()
                                                    .disabled(!gh_configured)
                                                    .on_click(move |_, _, cx| {
                                                        state_test.update(cx, |s, cx| {
                                                            s.run_background(
                                                                cx,
                                                                "Testing GitHub token connection...",
                                                                || {
                                                                    let tok = SecretService::get_token("github.com")?
                                                                        .ok_or_else(|| AppError::Validation("No GitHub token stored in keychain".to_string()))?;
                                                                    let client = GitHubClient::new("github.com", &tok);
                                                                    client.test_connection()
                                                                },
                                                                |s, msg, _| {
                                                                    s.set_notice(Some(msg));
                                                                },
                                                            );
                                                        });
                                                    })
                                            })
                                            .when(gh_configured, |row| {
                                                let holder_clone = holder_for_remote.clone();
                                                let state_clone = self.state.clone();
                                                row.child(
                                                    Button::new("btn_remove_gh_token")
                                                        .icon(IconName::Trash)
                                                        .label("Remove")
                                                        .small()
                                                        .ghost()
                                                        .on_click(move |_, _, cx| {
                                                            if let Err(e) = SecretService::delete_token("github.com") {
                                                                state_clone.update(cx, |s, cx| {
                                                                    s.set_error(Some(e.to_string()));
                                                                    cx.notify();
                                                                });
                                                            } else {
                                                                holder_clone.update(cx, |h, cx| {
                                                                    h.gh_configured = false;
                                                                    cx.notify();
                                                                });
                                                                state_clone.update(cx, |s, cx| {
                                                                    s.set_notice(Some("GitHub token removed from Keychain.".to_string()));
                                                                    cx.notify();
                                                                });
                                                            }
                                                        }),
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .w_full()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&gh_token_input).mask_toggle().cleanable(true)),
                                    )
                                    .child({
                                        let gh_input_clone = gh_token_input.clone();
                                        let holder_clone = holder_for_remote.clone();
                                        let state_clone = self.state.clone();
                                        Button::new("btn_save_gh_token")
                                            .icon(IconName::SaveCheck)
                                            .label("Save Token")
                                            .small()
                                            .primary()
                                            .on_click(move |_, window, cx| {
                                                let val = gh_input_clone.read(cx).value().trim().to_string();
                                                if val.is_empty() {
                                                    state_clone.update(cx, |s, cx| {
                                                        s.set_error(Some("Token cannot be empty".to_string()));
                                                        cx.notify();
                                                    });
                                                    return;
                                                }
                                                if let Err(e) = SecretService::store_token("github.com", &val) {
                                                    state_clone.update(cx, |s, cx| {
                                                        s.set_error(Some(e.to_string()));
                                                        cx.notify();
                                                    });
                                                } else {
                                                    gh_input_clone.update(cx, |inp, cx| {
                                                        inp.set_value(String::new(), window, cx);
                                                    });
                                                    holder_clone.update(cx, |h, cx| {
                                                        h.gh_configured = true;
                                                        cx.notify();
                                                    });
                                                    state_clone.update(cx, |s, cx| {
                                                        s.set_notice(Some("GitHub token saved securely in Keychain.".to_string()));
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                    }),
                            ),
                    )
                    // GitLab section
                    .child(
                        v_flex()
                            .gap_2()
                            .p_3p5()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme.colors.border.opacity(0.7))
                            .bg(theme.colors.muted.opacity(0.15))
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
                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .text_color(theme.colors.foreground)
                                                    .child("GitLab (gitlab.com)"),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(if gl_configured {
                                                        theme.colors.primary.opacity(0.12)
                                                    } else {
                                                        theme.colors.muted.opacity(0.4)
                                                    })
                                                    .text_xs()
                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .text_color(if gl_configured {
                                                        theme.colors.primary
                                                    } else {
                                                        theme.colors.muted_foreground
                                                    })
                                                    .child(if gl_configured {
                                                        "Configured in Keychain"
                                                    } else {
                                                        "Not Configured"
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child({
                                                let state_test = self.state.clone();
                                                Button::new("btn_test_gl_token")
                                                    .icon(IconName::Plug)
                                                    .label("Test Connection")
                                                    .small()
                                                    .secondary()
                                                    .disabled(!gl_configured)
                                                    .on_click(move |_, _, cx| {
                                                        state_test.update(cx, |s, cx| {
                                                            s.run_background(
                                                                cx,
                                                                "Testing GitLab token connection...",
                                                                || {
                                                                    let tok = SecretService::get_token("gitlab.com")?
                                                                        .ok_or_else(|| AppError::Validation("No GitLab token stored in keychain".to_string()))?;
                                                                    let client = GitLabClient::new("gitlab.com", &tok);
                                                                    client.test_connection()
                                                                },
                                                                |s, msg, _| {
                                                                    s.set_notice(Some(msg));
                                                                },
                                                            );
                                                        });
                                                    })
                                            })
                                            .when(gl_configured, |row| {
                                                let holder_clone = holder_for_remote.clone();
                                                let state_clone = self.state.clone();
                                                row.child(
                                                    Button::new("btn_remove_gl_token")
                                                        .icon(IconName::Trash)
                                                        .label("Remove")
                                                        .small()
                                                        .ghost()
                                                        .on_click(move |_, _, cx| {
                                                            if let Err(e) = SecretService::delete_token("gitlab.com") {
                                                                state_clone.update(cx, |s, cx| {
                                                                    s.set_error(Some(e.to_string()));
                                                                    cx.notify();
                                                                });
                                                            } else {
                                                                holder_clone.update(cx, |h, cx| {
                                                                    h.gl_configured = false;
                                                                    cx.notify();
                                                                });
                                                                state_clone.update(cx, |s, cx| {
                                                                    s.set_notice(Some("GitLab token removed from Keychain.".to_string()));
                                                                    cx.notify();
                                                                });
                                                            }
                                                        }),
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .w_full()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&gl_token_input).mask_toggle().cleanable(true)),
                                    )
                                    .child({
                                        let gl_input_clone = gl_token_input.clone();
                                        let holder_clone = holder_for_remote.clone();
                                        let state_clone = self.state.clone();
                                        Button::new("btn_save_gl_token")
                                            .icon(IconName::SaveCheck)
                                            .label("Save Token")
                                            .small()
                                            .primary()
                                            .on_click(move |_, window, cx| {
                                                let val = gl_input_clone.read(cx).value().trim().to_string();
                                                if val.is_empty() {
                                                    state_clone.update(cx, |s, cx| {
                                                        s.set_error(Some("Token cannot be empty".to_string()));
                                                        cx.notify();
                                                    });
                                                    return;
                                                }
                                                if let Err(e) = SecretService::store_token("gitlab.com", &val) {
                                                    state_clone.update(cx, |s, cx| {
                                                        s.set_error(Some(e.to_string()));
                                                        cx.notify();
                                                    });
                                                } else {
                                                    gl_input_clone.update(cx, |inp, cx| {
                                                        inp.set_value(String::new(), window, cx);
                                                    });
                                                    holder_clone.update(cx, |h, cx| {
                                                        h.gl_configured = true;
                                                        cx.notify();
                                                    });
                                                    state_clone.update(cx, |s, cx| {
                                                        s.set_notice(Some("GitLab token saved securely in Keychain.".to_string()));
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                    }),
                            ),
                    ),
            )
    }
}
