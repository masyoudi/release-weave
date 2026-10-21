use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::sidebar::{
    Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem, SidebarToggleButton,
};
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Entity, FocusHandle, IntoElement, KeyDownEvent, MouseButton, Render, Window, div, px,
    rgba,
};

use crate::domain::release::Release;
use crate::services::release_service::ReleaseService;
use crate::storage::repositories::ReleaseRepo;
use crate::ui::changes::ChangesView;
use crate::ui::components::apply_plan_dialog::ApplyPlanDialog;
use crate::ui::components::confirm_dialog::ConfirmDialog;
use crate::ui::components::open_repo_dialog::OpenRepoDialog;
use crate::ui::history::HistoryView;
use crate::ui::overview::OverviewView;
use crate::ui::palette::view::CommandPaletteView;
use crate::ui::readiness::ReadinessView;
use crate::ui::release_notes::ReleaseNotesView;
use crate::ui::settings::SettingsView;
use crate::ui::state::{AppState, NavTab};
use crate::ui::versioning::VersioningView;

pub struct ShellView {
    pub state: Entity<AppState>,
    pub focus_handle: FocusHandle,
}

impl ShellView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn select_tab(&mut self, tab: NavTab, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.set_nav(tab, cx);
        });
    }

    pub fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let is_palette_shortcut =
            event.keystroke.modifiers.secondary() && event.keystroke.key.eq_ignore_ascii_case("k");
        if is_palette_shortcut {
            self.state.update(cx, |s, cx| {
                let show = !s.show_command_palette;
                s.toggle_command_palette(show, cx);
            });
            return;
        }

        if self.state.read(cx).show_command_palette {
            match event.keystroke.key.to_lowercase().as_str() {
                "escape" => {
                    self.state.update(cx, |s, cx| {
                        s.toggle_command_palette(false, cx);
                    });
                    return;
                }
                "up" | "arrowup" => {
                    self.state.update(cx, |s, cx| {
                        s.palette_select_previous(cx);
                    });
                    return;
                }
                "down" | "arrowdown" => {
                    self.state.update(cx, |s, cx| {
                        let projs = s.projects.clone();
                        let pkgs = s.packages.clone();
                        let cmds =
                            crate::ui::palette::registry::build_palette_commands(&projs, &pkgs);
                        let filtered = crate::ui::palette::filter::filter_items(
                            &s.command_palette_query,
                            &cmds,
                            |c| &c.title,
                            |c| &c.keywords,
                        );
                        s.palette_select_next(filtered.len(), cx);
                    });
                    return;
                }
                "enter" => {
                    self.state.update(cx, |s, cx| {
                        let projs = s.projects.clone();
                        let pkgs = s.packages.clone();
                        let cmds =
                            crate::ui::palette::registry::build_palette_commands(&projs, &pkgs);
                        let filtered = crate::ui::palette::filter::filter_items(
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
                    return;
                }
                _ => {}
            }
        }

        if event.keystroke.key.eq_ignore_ascii_case("escape") {
            self.state.update(cx, |s, cx| {
                if s.show_open_repo_dialog
                    || s.show_project_switcher
                    || s.show_package_selector
                    || s.show_finalize_confirm
                    || s.show_apply_plan_dialog
                    || s.show_tag_confirm
                    || s.show_command_palette
                {
                    s.show_open_repo_dialog = false;
                    s.show_project_switcher = false;
                    s.show_package_selector = false;
                    s.show_finalize_confirm = false;
                    s.show_apply_plan_dialog = false;
                    s.show_tag_confirm = false;
                    s.show_command_palette = false;
                    s.pending_file_plan = None;
                    cx.notify();
                }
            });
            return;
        }

        if event.keystroke.modifiers.control {
            match event.keystroke.key.as_str() {
                "1" => self.select_tab(NavTab::Overview, cx),
                "2" => self.select_tab(NavTab::Changes, cx),
                "3" => self.select_tab(NavTab::ReleaseNotes, cx),
                "4" => self.select_tab(NavTab::Versioning, cx),
                "5" => self.select_tab(NavTab::Readiness, cx),
                "6" => self.select_tab(NavTab::History, cx),
                "7" => self.select_tab(NavTab::Settings, cx),
                _ => {}
            }
        }
    }
}

impl Render for ShellView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_maximized = window.is_maximized();
        let theme = cx.theme();
        let app_state = self.state.read(cx);
        let active_nav = app_state.active_nav;
        let is_loading = app_state.is_loading;
        let loading_msg = app_state.loading_message.clone();
        let error_msg = app_state.error_message.clone();
        let status_notice = app_state.status_notice.clone();

        let project_name = app_state
            .active_project
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "No Project".to_string());

        let has_project = app_state.active_project.is_some();
        let show_open_repo_dialog = app_state.show_open_repo_dialog;
        let show_project_switcher = app_state.show_project_switcher;
        let show_package_selector = app_state.show_package_selector;
        let show_finalize_confirm = app_state.show_finalize_confirm;
        let show_apply_plan_dialog = app_state.show_apply_plan_dialog;
        let show_tag_confirm = app_state.show_tag_confirm;
        let show_command_palette = app_state.show_command_palette;
        let pending_file_plan = app_state.pending_file_plan.clone();
        let selected_plan_patch_idx = app_state.selected_plan_patch_idx;
        let sidebar_collapsed = app_state.sidebar_collapsed;
        let projects = app_state.projects.clone();
        let packages = app_state.packages.clone();
        let current_proj_id = app_state.active_project.as_ref().and_then(|p| p.id);
        let active_package_id = app_state.active_package_id;

        let nav_items = [
            NavTab::Overview,
            NavTab::Changes,
            NavTab::ReleaseNotes,
            NavTab::Versioning,
            NavTab::Readiness,
            NavTab::History,
            NavTab::Settings,
        ];

        let state_clone_theme = self.state.clone();
        let state_clone_clear_err = self.state.clone();
        let state_clone_clear_notice = self.state.clone();

        v_flex()
            .size_full()
            .relative()
            .bg(theme.colors.background)
            .text_color(theme.colors.foreground)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key_down(event, cx);
            }))
            .child(
                // Compact Top Navbar (Platform-Adaptive)
                {
                    let mut navbar = h_flex()
                        .w_full()
                        .h(px(38.0))
                        .border_b_1()
                        .border_color(theme.colors.title_bar_border)
                        .bg(theme.colors.title_bar)
                        .justify_between()
                        .items_center();

                    #[cfg(target_os = "macos")]
                    {
                        // Provide clearance for native macOS traffic lights (Close, Minimize, Maximize)
                        navbar = navbar.pl(px(80.0)).pr_3();
                    }

                    #[cfg(not(target_os = "macos"))]
                    {
                        navbar = navbar.px_3();
                    }

                    navbar
                        .child(
                            // Left: Sidebar Toggle, Logo & Project Switcher
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child({
                                    let state_sidebar = self.state.clone();
                                    SidebarToggleButton::new()
                                        .collapsed(sidebar_collapsed)
                                        .accessibility_label("Toggle Sidebar")
                                        .on_click(move |_, _, cx| {
                                            state_sidebar.update(cx, |s, cx| {
                                                s.toggle_sidebar(cx);
                                            });
                                        })
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(theme.colors.primary)
                                        .on_mouse_down(MouseButton::Left, |event, window, _| {
                                            if event.click_count == 2 {
                                                window.zoom_window();
                                            } else {
                                                window.start_window_move();
                                            }
                                        })
                                        .child("ReleaseWeave"),
                                )
                                .child(div().text_xs().text_color(theme.colors.border).child("|"))
                                .child({
                                    let state_proj = self.state.clone();
                                    Button::new("shell_project_btn")
                                        .icon(IconName::FolderGit2)
                                        .label(if has_project {
                                            project_name
                                        } else {
                                            "Open Repository...".to_string()
                                        })
                                        .small()
                                        .ghost()
                                        .tooltip("Switch or Open Repository")
                                        .on_click(move |_, _, cx| {
                                            state_proj.update(cx, |s, cx| {
                                                if s.active_project.is_some() && !s.projects.is_empty()
                                                {
                                                    s.toggle_project_switcher(
                                                        !s.show_project_switcher,
                                                        cx,
                                                    );
                                                } else {
                                                    s.toggle_open_repo_dialog(true, cx);
                                                }
                                            });
                                        })
                                })
                                .when(!packages.is_empty(), |el| {
                                    let state_pkg = self.state.clone();
                                    let active_pkg_name = active_package_id
                                        .and_then(|id| packages.iter().find(|p| p.id == Some(id)))
                                        .map(|p| p.name.clone())
                                        .unwrap_or_else(|| "All Packages".to_string());

                                    el.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.border)
                                            .child("/"),
                                    )
                                    .child(
                                        Button::new("shell_package_btn")
                                            .icon(IconName::Package)
                                            .label(active_pkg_name)
                                            .small()
                                            .ghost()
                                            .tooltip("Switch Monorepo Package")
                                            .on_click(move |_, _, cx| {
                                                state_pkg.update(cx, |s, cx| {
                                                    let show = !s.show_package_selector;
                                                    s.toggle_package_selector(show, cx);
                                                });
                                            }),
                                    )
                                }),
                        )
                        .child(
                            // Center: Draggable Titlebar Spacer
                            div()
                                .flex_1()
                                .h_full()
                                .on_mouse_down(MouseButton::Left, |event, window, _| {
                                    if event.click_count == 2 {
                                        window.zoom_window();
                                    } else {
                                        window.start_window_move();
                                    }
                                }),
                        )
                        .child(
                            // Right: Command Palette, Open Repo, Theme Toggle, and optional Window Controls
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child({
                                    let state_palette = self.state.clone();
                                    let shortcut_label = CommandPaletteView::shortcut_label();
                                    Button::new("shell_palette_btn")
                                        .icon(IconName::Search)
                                        .label(shortcut_label)
                                        .small()
                                        .ghost()
                                        .tooltip(format!("Command Palette ({shortcut_label})"))
                                        .on_click(move |_, _, cx| {
                                            state_palette.update(cx, |s, cx| {
                                                let show = !s.show_command_palette;
                                                s.toggle_command_palette(show, cx);
                                            });
                                        })
                                })
                                .child({
                                    let state_open = self.state.clone();
                                    Button::new("shell_open_repo_btn")
                                        .icon(IconName::FolderOpen)
                                        .small()
                                        .ghost()
                                        .tooltip("Open Local Repository")
                                        .on_click(move |_, _, cx| {
                                            state_open.update(cx, |s, cx| {
                                                s.toggle_open_repo_dialog(true, cx);
                                            });
                                        })
                                })
                                .child(
                                    Button::new("shell_theme_toggle")
                                        .icon(if theme.is_dark() {
                                            IconName::Sun
                                        } else {
                                            IconName::Moon
                                        })
                                        .small()
                                        .ghost()
                                        .tooltip(if theme.is_dark() {
                                            "Switch to Light Theme"
                                        } else {
                                            "Switch to Dark Theme"
                                        })
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
                                )
                                .when(cfg!(not(target_os = "macos")), |el| {
                                    el.child(
                                        div()
                                            .w(px(1.0))
                                            .h(px(16.0))
                                            .mx_1()
                                            .bg(theme.colors.border),
                                    )
                                    .child(
                                        Button::new("shell_minimize_btn")
                                            .icon(IconName::WindowMinimize)
                                            .small()
                                            .ghost()
                                            .tooltip("Minimize Window")
                                            .on_click(|_, window, _| {
                                                window.minimize_window();
                                            }),
                                    )
                                    .child(
                                        Button::new("shell_maximize_btn")
                                            .icon(if is_maximized {
                                                IconName::WindowRestore
                                            } else {
                                                IconName::WindowMaximize
                                            })
                                            .small()
                                            .ghost()
                                            .tooltip(if is_maximized {
                                                "Restore Window"
                                            } else {
                                                "Maximize Window"
                                            })
                                            .on_click(|_, window, _| {
                                                window.zoom_window();
                                            }),
                                    )
                                    .child(
                                        Button::new("shell_close_btn")
                                            .icon(IconName::WindowClose)
                                            .small()
                                            .ghost()
                                            .tooltip("Close Application")
                                            .on_click(|_, _, cx| {
                                                cx.quit();
                                            }),
                                    )
                                }),
                        )
                },
            )
            .child(
                // Main Workspace Layout
                h_flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(
                        // Sidebar Navigation (T-101)
                        Sidebar::new("app_sidebar")
                            .collapsed(sidebar_collapsed)
                            .child(
                                SidebarGroup::new("Workspace").child(
                                    SidebarMenu::new().children(nav_items.into_iter().map(
                                        |item| {
                                            let is_active = active_nav == item;
                                            let state_tab = self.state.clone();

                                            SidebarMenuItem::new(item.title())
                                                .icon(item.icon())
                                                .active(is_active)
                                                .on_click(move |_, _, cx| {
                                                    state_tab.update(cx, |s, cx| {
                                                        s.set_nav(item, cx);
                                                    });
                                                })
                                        },
                                    )),
                                ),
                            ),
                    )
                    .child(
                        // Active Page Content
                        div()
                            .id("active_page_container")
                            .flex_1()
                            .h_full()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .child(match active_nav {
                                NavTab::Overview => {
                                    OverviewView::new(self.state.clone()).into_any_element()
                                }
                                NavTab::Changes => {
                                    ChangesView::new(self.state.clone()).into_any_element()
                                }
                                NavTab::ReleaseNotes => {
                                    ReleaseNotesView::new(self.state.clone()).into_any_element()
                                }
                                NavTab::Versioning => {
                                    VersioningView::new(self.state.clone()).into_any_element()
                                }
                                NavTab::Readiness => {
                                    ReadinessView::new(self.state.clone()).into_any_element()
                                }
                                NavTab::History => {
                                    HistoryView::new(self.state.clone()).into_any_element()
                                }
                                NavTab::Settings => {
                                    SettingsView::new(self.state.clone()).into_any_element()
                                }
                            }),
                    ),
            )
            .when(is_loading, |this| {
                this.child(
                    div()
                        .id("shell_loading_overlay")
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x00000044))
                        .items_center()
                        .justify_center()
                        .child(
                            v_flex()
                                .p_6()
                                .rounded_xl()
                                .border_1()
                                .border_color(theme.colors.border)
                                .bg(theme.colors.background)
                                .items_center()
                                .gap_3()
                                .child(
                                    Icon::new(IconName::RefreshCw).text_color(theme.colors.primary),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(theme.colors.foreground)
                                        .child(
                                            loading_msg
                                                .unwrap_or_else(|| "Processing...".to_string()),
                                        ),
                                ),
                        ),
                )
            })
            .when_some(error_msg, |this, err| {
                let state_err = state_clone_clear_err.clone();
                this.child(
                    div()
                        .id("shell_error_toast")
                        .absolute()
                        .top(px(56.0))
                        .right(px(20.0))
                        .w(px(440.0))
                        .p_4()
                        .rounded_xl()
                        .border_1()
                        .border_color(theme.colors.danger)
                        .bg(theme.colors.background)
                        .gap_2()
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(IconName::TriangleAlert)
                                                .text_color(theme.colors.danger),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(theme.colors.danger)
                                                .child("Error Occurred"),
                                        ),
                                )
                                .child(
                                    Button::new("btn_dismiss_error")
                                        .icon(IconName::CircleX)
                                        .ghost()
                                        .on_click(move |_, _, cx| {
                                            state_err.update(cx, |s, _| s.clear_error());
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.colors.foreground)
                                .child(err),
                        ),
                )
            })
            .when_some(status_notice, |this, notice| {
                let state_notice = state_clone_clear_notice.clone();
                this.child(
                    div()
                        .id("shell_status_notice")
                        .absolute()
                        .bottom(px(20.0))
                        .right(px(20.0))
                        .w(px(380.0))
                        .p_3()
                        .rounded_xl()
                        .border_1()
                        .border_color(theme.colors.success)
                        .bg(theme.colors.background)
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(IconName::CircleCheck)
                                                .text_color(theme.colors.success),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                .text_color(theme.colors.foreground)
                                                .child(notice),
                                        ),
                                )
                                .child(
                                    Button::new("btn_dismiss_notice")
                                        .icon(IconName::CircleX)
                                        .small()
                                        .ghost()
                                        .on_click(move |_, _, cx| {
                                            state_notice.update(cx, |s, cx| {
                                                s.clear_notice();
                                                cx.notify();
                                            });
                                        }),
                                ),
                        ),
                )
            })
            .when(show_project_switcher, |this| {
                let state_close = self.state.clone();
                let state_open_new = self.state.clone();
                let select_cb = self.state.clone();

                let backdrop_bg = theme.overlay;

                this.child(
                    v_flex()
                        .id("project_switcher_backdrop")
                        .absolute()
                        .inset_0()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .bg(backdrop_bg)
                        .on_click(move |_, _, cx| {
                            state_close.update(cx, |s, cx| s.toggle_project_switcher(false, cx));
                        })
                        .child(
                            v_flex()
                                .id("project_switcher_card")
                                .w(px(460.0))
                                .p_5()
                                .rounded_xl()
                                .border_1()
                                .border_color(theme.colors.border)
                                .bg(theme.colors.group_box)
                                .shadow_2xl()
                                .gap_4()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(theme.colors.foreground)
                                                .child("Switch Repository"),
                                        )
                                        .child(
                                            Button::new("btn_switcher_open_new")
                                                .label("+ Open Another...")
                                                .secondary()
                                                .on_click(move |_, _, cx| {
                                                    state_open_new.update(cx, |s, cx| {
                                                        s.toggle_open_repo_dialog(true, cx);
                                                    });
                                                }),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .id("project_switcher_list_scroll")
                                        .max_h(px(240.0))
                                        .overflow_y_scrollbar()
                                        .gap_2()
                                        .children(projects.into_iter().map(|proj| {
                                            let is_current =
                                                proj.id.is_some() && proj.id == current_proj_id;
                                            let state_sel = select_cb.clone();
                                            let state_del = select_cb.clone();
                                            let p_clone = proj.clone();
                                            let proj_id = proj.id;
                                            let item_key = proj
                                                .id
                                                .map(|id| id.to_string())
                                                .unwrap_or_else(|| proj.name.clone());

                                            h_flex()
                                                .w_full()
                                                .p_3()
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(if is_current {
                                                    theme.colors.primary
                                                } else {
                                                    theme.colors.border
                                                })
                                                .bg(if is_current {
                                                    theme.colors.primary.opacity(0.1)
                                                } else {
                                                    theme.colors.group_box
                                                })
                                                .justify_between()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    h_flex()
                                                        .id(format!("switcher_item_{item_key}"))
                                                        .flex_1()
                                                        .justify_between()
                                                        .items_center()
                                                        .cursor_pointer()
                                                        .on_click(move |_, _, cx| {
                                                            state_sel.update(cx, |s, cx| {
                                                                s.select_project(
                                                                    p_clone.clone(),
                                                                    cx,
                                                                );
                                                            });
                                                        })
                                                        .child(
                                                            v_flex()
                                                                .gap(px(2.0))
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(
                                                                            gpui_kit::FontWeight::SEMIBOLD,
                                                                        )
                                                                        .text_color(theme.colors.foreground)
                                                                        .child(proj.name),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            theme.colors.muted_foreground,
                                                                        )
                                                                        .child(
                                                                            proj.repository_path
                                                                                .to_string_lossy()
                                                                                .to_string(),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.colors.muted_foreground)
                                                                .child(format!(
                                                                    "branch: {}",
                                                                    proj.default_branch
                                                                )),
                                                        ),
                                                )
                                                .when_some(proj_id, |row, id| {
                                                    row.child(
                                                        Button::new(format!("switcher_delete_{id}"))
                                                            .icon(IconName::Trash)
                                                            .small()
                                                            .ghost()
                                                            .tooltip("Remove Repository")
                                                            .on_click(move |_, _, cx| {
                                                                state_del.update(cx, |s, cx| {
                                                                    s.remove_project(id, cx);
                                                                });
                                                            }),
                                                    )
                                                })
                                        })),
                                ),
                        ),
                )
            })
            .when(show_package_selector, |this| {
                let state_close = self.state.clone();
                let state_btn_close = self.state.clone();
                let select_cb = self.state.clone();
                let backdrop_bg = theme.overlay;

                this.child(
                    v_flex()
                        .id("package_selector_backdrop")
                        .absolute()
                        .inset_0()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .bg(backdrop_bg)
                        .on_click(move |_, _, cx| {
                            state_close.update(cx, |s, cx| s.toggle_package_selector(false, cx));
                        })
                        .child(
                            v_flex()
                                .id("package_selector_card")
                                .w(px(460.0))
                                .p_5()
                                .rounded_xl()
                                .border_1()
                                .border_color(theme.colors.border)
                                .bg(theme.colors.group_box)
                                .shadow_2xl()
                                .gap_4()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(theme.colors.foreground)
                                                .child("Select Workspace Package"),
                                        )
                                        .child(
                                            Button::new("btn_pkg_close")
                                                .icon(IconName::CircleX)
                                                .small()
                                                .ghost()
                                                .on_click(move |_, _, cx| {
                                                    state_btn_close.update(cx, |s, cx| {
                                                        s.toggle_package_selector(false, cx);
                                                    });
                                                }),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .id("package_selector_list_scroll")
                                        .max_h(px(260.0))
                                        .overflow_y_scrollbar()
                                        .gap_2()
                                        .child({
                                            let is_all = active_package_id.is_none();
                                            let state_all = select_cb.clone();
                                            h_flex()
                                                .id("package_item_all")
                                                .w_full()
                                                .p_3()
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(if is_all {
                                                    theme.colors.primary
                                                } else {
                                                    theme.colors.border
                                                })
                                                .bg(if is_all {
                                                    theme.colors.primary.opacity(0.1)
                                                } else {
                                                    theme.colors.group_box
                                                })
                                                .justify_between()
                                                .items_center()
                                                .cursor_pointer()
                                                .on_click(move |_, _, cx| {
                                                    state_all.update(cx, |s, cx| {
                                                        s.select_package(None, cx);
                                                    });
                                                })
                                                .child(
                                                    v_flex()
                                                        .gap(px(2.0))
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                .text_color(theme.colors.foreground)
                                                                .child("All Packages (Root Repository)"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.colors.muted_foreground)
                                                                .child("Include commits and releases across entire workspace"),
                                                        ),
                                                )
                                        })
                                        .children(packages.into_iter().map(|pkg| {
                                            let is_current =
                                                pkg.id.is_some() && pkg.id == active_package_id;
                                            let state_sel = select_cb.clone();
                                            let pkg_id = pkg.id;
                                            let pkg_name = pkg.name.clone();
                                            let rel_path = pkg.relative_path.clone();
                                            let ver_str = pkg
                                                .current_version
                                                .clone()
                                                .unwrap_or_else(|| "unversioned".to_string());
                                            let item_key = pkg
                                                .id
                                                .map(|id| id.to_string())
                                                .unwrap_or_else(|| pkg_name.clone());

                                            h_flex()
                                                .id(format!("package_item_{item_key}"))
                                                .w_full()
                                                .p_3()
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(if is_current {
                                                    theme.colors.primary
                                                } else {
                                                    theme.colors.border
                                                })
                                                .bg(if is_current {
                                                    theme.colors.primary.opacity(0.1)
                                                } else {
                                                    theme.colors.group_box
                                                })
                                                .justify_between()
                                                .items_center()
                                                .cursor_pointer()
                                                .on_click(move |_, _, cx| {
                                                    state_sel.update(cx, |s, cx| {
                                                        s.select_package(pkg_id, cx);
                                                    });
                                                })
                                                .child(
                                                    v_flex()
                                                        .gap(px(2.0))
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                .text_color(theme.colors.foreground)
                                                                .child(pkg_name),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.colors.muted_foreground)
                                                                .child(rel_path),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.colors.muted_foreground)
                                                        .child(format!("v{ver_str}")),
                                                )
                                        })),
                                ),
                        ),
                )
            })
            .when(show_open_repo_dialog, |this| {
                let state_cancel = self.state.clone();
                let state_dir = self.state.clone();
                let state_select = self.state.clone();
                let state_remove = self.state.clone();
                let projects_for_dialog = self.state.read(cx).projects.clone();

                this.child(
                    OpenRepoDialog::new(move |_, _, cx| {
                        state_cancel.update(cx, |s, cx| s.toggle_open_repo_dialog(false, cx));
                    })
                    .with_recent_projects(projects_for_dialog)
                    .on_open_directory(move |_, _, cx| {
                        state_dir.update(cx, |s, cx| {
                            s.pick_and_open_directory(cx);
                        });
                    })
                    .on_select_project(move |proj, _, cx| {
                        state_select.update(cx, |s, cx| {
                            s.select_project(proj, cx);
                        });
                    })
                    .on_remove_project(move |proj_id, _, cx| {
                        state_remove.update(cx, |s, cx| {
                            s.remove_project(proj_id, cx);
                        });
                    }),
                )
            })
            .when(show_finalize_confirm, |this| {
                let state_confirm = self.state.clone();
                let state_cancel = self.state.clone();
                let s_read = self.state.read(cx);
                let db_path = s_read.db_path.clone();
                let project_id = s_read.active_project.as_ref().and_then(|p| p.id).unwrap_or(1);
                let target_ver = s_read.target_version.clone();
                let prev_ver = s_read.current_version.clone();
                let start_commit = s_read
                    .commit_range
                    .as_ref()
                    .and_then(|r| r.from.map(|o| o.to_string()));
                let end_commit = s_read
                    .commit_range
                    .as_ref()
                    .map(|r| r.to.to_string())
                    .or_else(|| s_read.commits.first().map(|c| c.hash.clone()))
                    .unwrap_or_else(|| "HEAD".to_string());
                let active_package_id = s_read.active_package_id;
                let final_notes = s_read.release_notes.clone();
                let items = s_read.change_items.clone();
                let checks = s_read.readiness_checks.clone();

                this.child(
                    ConfirmDialog::new(
                        "Finalize Release",
                        "Release status will be set to Final and recorded in the SQLite database. Git files and tags will remain untouched.",
                        move |_, _, cx| {
                            let db_path = db_path.clone();
                            let target_ver = target_ver.clone();
                            let prev_ver = prev_ver.clone();
                            let start_commit = start_commit.clone();
                            let end_commit = end_commit.clone();
                            let final_notes = final_notes.clone();
                            let items = items.clone();
                            let checks = checks.clone();
                            state_confirm.update(cx, |s, cx| {
                                s.show_finalize_confirm = false;
                                s.run_background(
                                    cx,
                                    "Finalizing release in SQLite database...",
                                    move || {
                                        let mut conn = crate::storage::database::open_path(&db_path)?;
                                        let mut draft_rel = Release::new_draft(
                                            project_id,
                                            &target_ver,
                                            prev_ver,
                                            start_commit,
                                            &end_commit,
                                        );
                                        draft_rel.package_id = active_package_id;
                                        let rid = ReleaseRepo::insert(&conn, &draft_rel)?;
                                        let finalized = ReleaseService::finalize_release(
                                            &mut conn,
                                            rid,
                                            &final_notes,
                                            &items,
                                            &checks,
                                        )?;
                                        let all_rels = if let Some(pkg_id) = active_package_id {
                                            ReleaseRepo::list_by_package(&conn, project_id, pkg_id)?
                                        } else {
                                            ReleaseRepo::list_by_project(&conn, project_id)?
                                        };
                                        Ok((finalized, all_rels))
                                    },
                                    |s, (finalized, all_rels), _| {
                                        s.active_release = Some(finalized.clone());
                                        s.history_releases = all_rels;
                                        s.active_nav = NavTab::History;
                                        s.set_notice(Some(format!(
                                            "Release v{} finalized and saved to history.",
                                            finalized.version
                                        )));
                                    },
                                );
                            });
                        },
                        move |_, _, cx| {
                            state_cancel.update(cx, |s, cx| {
                                s.show_finalize_confirm = false;
                                cx.notify();
                            });
                        },
                    )
                    .with_labels("Finalize", "Cancel"),
                )
            })
            .when(show_apply_plan_dialog && pending_file_plan.is_some(), |this| {
                let state_confirm = self.state.clone();
                let state_cancel = self.state.clone();
                let state_select = self.state.clone();
                let plan = pending_file_plan.unwrap();

                this.child(
                    ApplyPlanDialog::new(
                        plan,
                        selected_plan_patch_idx,
                        move |_, _, cx| {
                            state_confirm.update(cx, |s, cx| {
                                s.apply_pending_file_plan_sync(cx);
                            });
                        },
                        move |_, _, cx| {
                            state_cancel.update(cx, |s, cx| {
                                s.toggle_apply_plan_dialog(false, cx);
                            });
                        },
                    )
                    .on_select_patch(move |idx, _, cx| {
                        state_select.update(cx, |s, cx| {
                            s.select_plan_patch(idx, cx);
                        });
                    }),
                )
            })
            .when(show_tag_confirm, |this| {
                let state_confirm = self.state.clone();
                let state_cancel = self.state.clone();
                let s_read = self.state.read(cx);
                let tag_name = if s_read.target_version.starts_with('v') {
                    s_read.target_version.clone()
                } else {
                    format!("v{}", s_read.target_version)
                };
                let head_commit = s_read
                    .commits
                    .first()
                    .map(|c| c.hash.chars().take(7).collect::<String>())
                    .unwrap_or_else(|| "HEAD".to_string());

                let title = format!("Create Local Git Tag '{tag_name}'");
                let msg = format!(
                    "Create an annotated Git tag '{tag_name}' on commit {head_commit}. This tag is created purely locally and is never pushed to remote servers."
                );

                this.child(
                    ConfirmDialog::new(
                        title,
                        msg,
                        move |_, _, cx| {
                            state_confirm.update(cx, |s, cx| {
                                s.create_git_tag_sync(cx);
                            });
                        },
                        move |_, _, cx| {
                            state_cancel.update(cx, |s, cx| {
                                s.show_tag_confirm = false;
                                cx.notify();
                            });
                        },
                    )
                    .with_labels("Create Tag", "Cancel"),
                )
            })
            .when(show_command_palette, |this| {
                this.child(CommandPaletteView::new(self.state.clone()))
            })
    }
}
