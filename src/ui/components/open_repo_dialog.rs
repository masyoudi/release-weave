use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClickEvent, IntoElement, RenderOnce, Window, div, px};
use std::path::PathBuf;
use std::rc::Rc;

use crate::domain::project::Project;

pub type DialogActionCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
pub type ProjectSelectCallback = Rc<dyn Fn(Project, &mut Window, &mut App) + 'static>;
pub type ProjectRemoveCallback = Rc<dyn Fn(i64, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct OpenRepoDialog {
    current_dir: Option<PathBuf>,
    recent_projects: Vec<Project>,
    on_open_directory: Option<DialogActionCallback>,
    on_select_project: Option<ProjectSelectCallback>,
    on_remove_project: Option<ProjectRemoveCallback>,
    on_cancel: DialogActionCallback,
}

impl OpenRepoDialog {
    pub fn new<FCancel>(on_cancel: FCancel) -> Self
    where
        FCancel: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        let cwd = std::env::current_dir().ok();
        Self {
            current_dir: cwd,
            recent_projects: Vec::new(),
            on_open_directory: None,
            on_select_project: None,
            on_remove_project: None,
            on_cancel: Rc::new(on_cancel),
        }
    }

    pub fn with_recent_projects(mut self, projects: Vec<Project>) -> Self {
        self.recent_projects = projects;
        self
    }

    pub fn on_open_directory<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_open_directory = Some(Rc::new(callback));
        self
    }

    pub fn on_open_cwd<F>(self, callback: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_open_directory(callback)
    }

    pub fn on_select_project<F>(mut self, callback: F) -> Self
    where
        F: Fn(Project, &mut Window, &mut App) + 'static,
    {
        self.on_select_project = Some(Rc::new(callback));
        self
    }

    pub fn on_remove_project<F>(mut self, callback: F) -> Self
    where
        F: Fn(i64, &mut Window, &mut App) + 'static,
    {
        self.on_remove_project = Some(Rc::new(callback));
        self
    }
}

impl RenderOnce for OpenRepoDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let cancel_cb = self.on_cancel.clone();
        let cancel_cb_backdrop = cancel_cb.clone();

        let open_dir_cb = self.on_open_directory.clone();

        let backdrop_bg = theme.overlay;

        v_flex()
            .id("open_repo_dialog_backdrop")
            .absolute()
            .inset_0()
            .size_full()
            .items_center()
            .justify_center()
            .bg(backdrop_bg)
            .on_click(move |ev, win, cx| cancel_cb_backdrop(ev, win, cx))
            .child(
                v_flex()
                    .id("open_repo_dialog_content")
                    .w(px(520.0))
                    .max_h(px(560.0))
                    .p_6()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .shadow_2xl()
                    .gap_5()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        // Header
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(theme.colors.foreground)
                                    .child("Open Local Git Repository"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Select a local Git repository to start release management."),
                            ),
                    )
                    .child(
                        // Directory picker action
                        v_flex()
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.group_box)
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Directory Selection"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.group_box_foreground)
                                    .child("Choose a repository directory using your system file manager."),
                            )
                            .child(if let Some(handler) = open_dir_cb {
                                Button::new("btn_dialog_open_dir")
                                    .icon(IconName::FolderOpen)
                                    .label("Open Directory")
                                    .primary()
                                    .on_click(move |ev, win, cx| handler(ev, win, cx))
                            } else {
                                Button::new("btn_dialog_open_dir_disabled")
                                    .icon(IconName::FolderOpen)
                                    .label("Open Directory")
                                    .secondary()
                            }),
                    )
                    .when(!self.recent_projects.is_empty(), |this| {
                        let select_cb = self.on_select_project.clone();
                        let remove_cb = self.on_remove_project.clone();
                        this.child(
                            v_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(theme.colors.muted_foreground)
                                        .child("Recent Repositories"),
                                )
                                .child(
                                    v_flex()
                                        .id("dialog_recent_repos_scroll")
                                        .max_h(px(160.0))
                                        .overflow_y_scrollbar()
                                        .gap_1()
                                        .children(self.recent_projects.into_iter().take(5).map(|proj| {
                                            let cb_clone = select_cb.clone();
                                            let rm_clone = remove_cb.clone();
                                            let proj_clone = proj.clone();
                                            let proj_id = proj.id;
                                            let item_key = proj
                                                .id
                                                .map(|id| id.to_string())
                                                .unwrap_or_else(|| proj.name.clone());

                                            h_flex()
                                                .w_full()
                                                .p_2()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(theme.colors.border)
                                                .bg(theme.colors.group_box)
                                                .justify_between()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    h_flex()
                                                        .id(format!("open_dialog_recent_{item_key}"))
                                                        .flex_1()
                                                        .justify_between()
                                                        .items_center()
                                                        .cursor_pointer()
                                                        .on_click(move |_, win, cx| {
                                                            if let Some(cb) = &cb_clone {
                                                                cb(proj_clone.clone(), win, cx);
                                                            }
                                                        })
                                                        .child(
                                                            v_flex()
                                                                .gap(px(1.0))
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .text_color(theme.colors.foreground)
                                                                        .child(proj.name.clone()),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(theme.colors.muted_foreground)
                                                                        .child(proj.repository_path.to_string_lossy().to_string()),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.colors.muted_foreground)
                                                                .child(format!("branch: {}", proj.default_branch)),
                                                        ),
                                                )
                                                .when_some(proj_id, |row, id| {
                                                    row.child(
                                                        Button::new(format!(
                                                            "open_dialog_remove_{id}"
                                                        ))
                                                        .icon(IconName::Trash)
                                                        .small()
                                                        .ghost()
                                                        .tooltip("Remove from Recent Repositories")
                                                        .on_click(move |_, win, cx| {
                                                            if let Some(rm) = &rm_clone {
                                                                rm(id, win, cx);
                                                            }
                                                        }),
                                                    )
                                                })
                                        })),
                                ),
                        )
                    })
                    .child(
                        // Footer buttons
                        h_flex()
                            .w_full()
                            .justify_end()
                            .child(
                                Button::new("btn_dialog_cancel")
                                    .label("Cancel")
                                    .secondary()
                                    .on_click(move |ev, win, cx| cancel_cb(ev, win, cx)),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_repo_dialog_constructs() {
        let dialog = OpenRepoDialog::new(|_, _, _| {})
            .with_recent_projects(Vec::new())
            .on_open_directory(|_, _, _| {});
        assert!(dialog.current_dir.is_some());
        assert!(dialog.recent_projects.is_empty());
        assert!(dialog.on_open_directory.is_some());
    }
}
