use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClickEvent, IntoElement, RenderOnce, Window, div, px};
use std::rc::Rc;

pub type DialogCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct ConfirmDialog {
    title: String,
    message: String,
    confirm_label: String,
    cancel_label: String,
    is_destructive: bool,
    on_confirm: DialogCallback,
    on_cancel: DialogCallback,
}

impl ConfirmDialog {
    pub fn new<FConfirm, FCancel>(
        title: impl Into<String>,
        message: impl Into<String>,
        on_confirm: FConfirm,
        on_cancel: FCancel,
    ) -> Self
    where
        FConfirm: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        FCancel: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: "Confirm".to_string(),
            cancel_label: "Cancel".to_string(),
            is_destructive: false,
            on_confirm: Rc::new(on_confirm),
            on_cancel: Rc::new(on_cancel),
        }
    }

    pub fn destructive(mut self, destructive: bool) -> Self {
        self.is_destructive = destructive;
        self
    }

    pub fn with_labels(mut self, confirm: impl Into<String>, cancel: impl Into<String>) -> Self {
        self.confirm_label = confirm.into();
        self.cancel_label = cancel.into();
        self
    }

    /// Evaluates keyboard shortcuts (e.g. Escape to safely dismiss)
    pub fn should_cancel_on_key(key: &str) -> bool {
        key.eq_ignore_ascii_case("escape")
    }
}

impl RenderOnce for ConfirmDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let cancel_cb = self.on_cancel.clone();
        let confirm_cb = self.on_confirm.clone();

        let mut confirm_btn = Button::new("dialog_confirm_btn")
            .icon(if self.is_destructive {
                IconName::TriangleAlert
            } else {
                IconName::Check
            })
            .label(self.confirm_label)
            .on_click(move |ev, win, cx| confirm_cb(ev, win, cx));

        if self.is_destructive {
            confirm_btn = confirm_btn.danger();
        } else {
            confirm_btn = confirm_btn.primary();
        }

        let cancel_cb_backdrop = cancel_cb.clone();
        let backdrop_bg = theme.overlay;

        v_flex()
            .id("confirm_dialog_backdrop")
            .absolute()
            .inset_0()
            .size_full()
            .items_center()
            .justify_center()
            .bg(backdrop_bg)
            .on_click(move |ev, win, cx| cancel_cb_backdrop(ev, win, cx))
            .child(
                v_flex()
                    .id("confirm_dialog_content")
                    .w(px(420.0))
                    .p_6()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .bg(theme.colors.group_box)
                    .shadow_2xl()
                    .gap_4()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(theme.colors.foreground)
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.colors.muted_foreground)
                            .child(self.message),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_end()
                            .gap_3()
                            .child(
                                Button::new("dialog_cancel_btn")
                                    .icon(IconName::X)
                                    .label(self.cancel_label)
                                    .secondary()
                                    .on_click(move |ev, win, cx| cancel_cb(ev, win, cx)),
                            )
                            .child(confirm_btn),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn confirm_dialog_construction_and_escape_cancellation() {
        assert!(ConfirmDialog::should_cancel_on_key("Escape"));
        assert!(ConfirmDialog::should_cancel_on_key("escape"));
        assert!(!ConfirmDialog::should_cancel_on_key("Enter"));
        assert!(!ConfirmDialog::should_cancel_on_key("a"));

        let cancelled = Rc::new(Cell::new(false));
        let confirmed = Rc::new(Cell::new(false));

        let c_cancelled = cancelled.clone();
        let c_confirmed = confirmed.clone();

        let dialog = ConfirmDialog::new(
            "Delete Draft",
            "Are you sure?",
            move |_, _, _| c_confirmed.set(true),
            move |_, _, _| c_cancelled.set(true),
        )
        .destructive(true)
        .with_labels("Delete", "Cancel");

        assert_eq!(dialog.confirm_label, "Delete");
        assert_eq!(dialog.cancel_label, "Cancel");
        assert!(dialog.is_destructive);
    }
}
