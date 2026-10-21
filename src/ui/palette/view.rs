use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Icon, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, FontWeight, IntoElement, Render, RenderOnce, Subscription, Window, div,
    px,
};

use crate::ui::palette::filter::filter_items;
use crate::ui::palette::registry::build_palette_commands;
use crate::ui::state::AppState;

struct PaletteInputHolder {
    input: Entity<InputState>,
    _sub: Subscription,
}

impl Render for PaletteInputHolder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct CommandPaletteView {
    state: Entity<AppState>,
}

impl CommandPaletteView {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }

    pub fn shortcut_label() -> &'static str {
        if cfg!(target_os = "macos") {
            "⌘K"
        } else {
            "Ctrl+K"
        }
    }
}

impl RenderOnce for CommandPaletteView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let app_state = self.state.read(cx);

        let query = app_state.command_palette_query.clone();
        let selected_idx = app_state.command_palette_selected_index;
        let recent_projects = app_state.projects.clone();
        let packages = app_state.packages.clone();

        let all_commands = build_palette_commands(&recent_projects, &packages);
        let scored_commands = filter_items(&query, &all_commands, |c| &c.title, |c| &c.keywords);

        let state_for_sub = self.state.clone();
        let state_for_close = self.state.clone();

        let input_holder = window.use_state(cx, move |window, cx| {
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder("Type a command or search repository...")
            });
            let state_sub = state_for_sub.clone();
            let sub = cx.subscribe(&input, move |_, input, event, cx| match event {
                InputEvent::Change => {
                    let val = input.read(cx).value().to_string();
                    state_sub.update(cx, |s, cx| {
                        s.set_palette_query(val, cx);
                    });
                }
                InputEvent::PressEnter { .. } => {
                    state_sub.update(cx, |s, cx| {
                        let projs = s.projects.clone();
                        let pkgs = s.packages.clone();
                        let cmds = build_palette_commands(&projs, &pkgs);
                        let filtered = filter_items(
                            &s.command_palette_query,
                            &cmds,
                            |c| &c.title,
                            |c| &c.keywords,
                        );
                        if !filtered.is_empty() {
                            let idx = s.command_palette_selected_index.min(filtered.len() - 1);
                            let action = filtered[idx].item.action.clone();
                            s.execute_palette_action(action, cx);
                        } else {
                            s.toggle_command_palette(false, cx);
                        }
                    });
                }
                _ => {}
            });
            PaletteInputHolder { input, _sub: sub }
        });

        // Ensure search input gets keyboard focus
        let input_entity = input_holder.read(cx).input.clone();
        input_entity.update(cx, |inp, cx| {
            inp.focus(window, cx);
        });

        let theme = cx.theme();
        let backdrop_bg = theme.overlay;
        let shortcut_hint = if cfg!(target_os = "macos") {
            "⌘K"
        } else {
            "Ctrl+K"
        };

        v_flex()
            .id("command_palette_backdrop")
            .absolute()
            .inset_0()
            .size_full()
            .items_center()
            .justify_start()
            .pt(px(70.0))
            .bg(backdrop_bg)
            .on_click(move |_, _, cx| {
                state_for_close.update(cx, |s, cx| {
                    s.toggle_command_palette(false, cx);
                });
            })
            .child(
                v_flex()
                    .id("command_palette_content")
                    .w(px(580.0))
                    .max_h(px(480.0))
                    .bg(theme.colors.group_box)
                    .border_1()
                    .border_color(theme.colors.border)
                    .rounded_xl()
                    .shadow_lg()
                    .on_click(|_, _, cx| cx.stop_propagation()) // Prevent backdrop dismiss when clicking inside
                    .child(
                        // Header with input and keyboard hints
                        h_flex()
                            .w_full()
                            .items_center()
                            .px_3()
                            .py_2p5()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.colors.border)
                            .child(
                                Icon::new(IconName::Search)
                                    .size_4()
                                    .text_color(theme.colors.muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&input_entity).cleanable(true)),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme.colors.muted)
                                            .text_color(theme.colors.muted_foreground)
                                            .child("ESC"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme.colors.muted)
                                            .text_color(theme.colors.muted_foreground)
                                            .child(shortcut_hint),
                                    ),
                            ),
                    )
                    .child(
                        // Results List
                        v_flex()
                            .id("command_palette_scroll_list")
                            .w_full()
                            .max_h(px(380.0))
                            .overflow_y_scrollbar()
                            .p_2()
                            .gap_1()
                            .when(scored_commands.is_empty(), |el| {
                                el.child(
                                    div()
                                        .w_full()
                                        .py_8()
                                        .items_center()
                                        .justify_center()
                                        .text_center()
                                        .text_sm()
                                        .text_color(theme.colors.muted_foreground)
                                        .child("No matching commands or repositories found."),
                                )
                            })
                            .children(scored_commands.into_iter().enumerate().map(
                                |(idx, scored)| {
                                    let cmd = scored.item.clone();
                                    let is_selected = idx == selected_idx;
                                    let state_item = self.state.clone();
                                    let action = cmd.action.clone();

                                    h_flex()
                                        .id(format!("palette_cmd_{}", cmd.id))
                                        .w_full()
                                        .px_3()
                                        .py_2()
                                        .rounded_lg()
                                        .items_center()
                                        .justify_between()
                                        .cursor_pointer()
                                        .when(is_selected, |el| el.bg(theme.colors.accent))
                                        .text_color(if is_selected {
                                            theme.colors.accent_foreground
                                        } else {
                                            theme.colors.foreground
                                        })
                                        .on_click(move |_, _, cx| {
                                            let act = action.clone();
                                            state_item.update(cx, |s, cx| {
                                                s.execute_palette_action(act, cx);
                                            });
                                        })
                                        .child(
                                            h_flex()
                                                .gap_2p5()
                                                .items_center()
                                                .child(
                                                    Icon::new(
                                                        cmd.icon.unwrap_or(IconName::Sparkles),
                                                    )
                                                    .size_4()
                                                    .text_color(if is_selected {
                                                        theme.colors.primary
                                                    } else {
                                                        theme.colors.muted_foreground
                                                    }),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(if is_selected {
                                                            FontWeight::MEDIUM
                                                        } else {
                                                            FontWeight::NORMAL
                                                        })
                                                        .child(cmd.title.clone()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(if is_selected {
                                                    theme.colors.background
                                                } else {
                                                    theme.colors.muted
                                                })
                                                .text_color(theme.colors.muted_foreground)
                                                .child(cmd.category.clone()),
                                        )
                                },
                            )),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_label_matches_target_platform() {
        let label = CommandPaletteView::shortcut_label();
        if cfg!(target_os = "macos") {
            assert_eq!(label, "⌘K");
        } else {
            assert_eq!(label, "Ctrl+K");
        }
    }
}
