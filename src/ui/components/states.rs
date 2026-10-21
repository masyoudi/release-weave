use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClickEvent, IntoElement, RenderOnce, Window, div, px};
use std::rc::Rc;

pub type StateActionCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct EmptyStateView {
    title: String,
    description: String,
    action_label: Option<String>,
    on_action: Option<StateActionCallback>,
}

impl EmptyStateView {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            action_label: None,
            on_action: None,
        }
    }

    pub fn with_action<F>(mut self, label: impl Into<String>, on_action: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.action_label = Some(label.into());
        self.on_action = Some(Rc::new(on_action));
        self
    }
}

impl RenderOnce for EmptyStateView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mut view = v_flex()
            .items_center()
            .justify_center()
            .p_8()
            .gap_3()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(theme.colors.foreground)
                    .child(self.title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.muted_foreground)
                    .child(self.description),
            );

        if let (Some(label), Some(handler)) = (self.action_label, self.on_action) {
            view = view.child(
                Button::new("empty_action_btn")
                    .label(label)
                    .primary()
                    .on_click(move |ev, win, cx| handler(ev, win, cx)),
            );
        }

        view
    }
}

#[derive(IntoElement)]
pub struct LoadingStateView {
    message: String,
}

impl LoadingStateView {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl RenderOnce for LoadingStateView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .items_center()
            .justify_center()
            .p_8()
            .gap_3()
            .child(Icon::new(IconName::RefreshCw).text_color(theme.colors.primary))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(theme.colors.muted_foreground)
                    .child(self.message),
            )
    }
}

#[derive(IntoElement)]
pub struct ErrorStateView {
    title: String,
    cause: String,
    recovery_label: String,
    on_recovery: Option<StateActionCallback>,
}

impl ErrorStateView {
    pub fn new(title: impl Into<String>, cause: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            cause: cause.into(),
            recovery_label: "Retry".to_string(),
            on_recovery: None,
        }
    }

    pub fn with_recovery<F>(mut self, label: impl Into<String>, on_recovery: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.recovery_label = label.into();
        self.on_recovery = Some(Rc::new(on_recovery));
        self
    }
}

impl RenderOnce for ErrorStateView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mut view = v_flex()
            .items_center()
            .justify_center()
            .p_8()
            .gap_4()
            .child(Icon::new(IconName::TriangleAlert).text_color(theme.colors.danger))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(theme.colors.danger)
                    .child(self.title),
            )
            .child(
                div()
                    .max_w(px(500.0))
                    .p_3()
                    .rounded_md()
                    .bg(theme.colors.group_box)
                    .border_1()
                    .border_color(theme.colors.border)
                    .text_xs()
                    .text_color(theme.colors.group_box_foreground)
                    .child(self.cause),
            );

        if let Some(handler) = self.on_recovery {
            view = view.child(
                Button::new("error_recovery_btn")
                    .icon(IconName::RefreshCw)
                    .label(self.recovery_label)
                    .secondary()
                    .on_click(move |ev, win, cx| handler(ev, win, cx)),
            );
        }

        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_views_construct_with_details() {
        let empty = EmptyStateView::new("No project", "Add a Git repository");
        assert_eq!(empty.title, "No project");
        assert_eq!(empty.description, "Add a Git repository");

        let loading = LoadingStateView::new("Checking git status...");
        assert_eq!(loading.message, "Checking git status...");

        let error = ErrorStateView::new("Failed to read commits", "I/O error: permission denied")
            .with_recovery("Recover", |_, _, _| {});
        assert_eq!(error.title, "Failed to read commits");
        assert_eq!(error.cause, "I/O error: permission denied");
        assert_eq!(error.recovery_label, "Recover");
        assert!(error.on_recovery.is_some());
    }
}
