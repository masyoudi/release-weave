use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, IntoElement, RenderOnce, Window, div, px};

#[derive(IntoElement)]
pub struct DiffView {
    patch: String,
    truncated: bool,
}

impl DiffView {
    pub fn new(patch: impl Into<String>, truncated: bool) -> Self {
        Self {
            patch: patch.into(),
            truncated,
        }
    }
}

impl RenderOnce for DiffView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let mut container = v_flex()
            .min_w_full()
            .p_3()
            .rounded_md()
            .bg(theme.colors.group_box)
            .border_1()
            .border_color(theme.colors.border)
            .gap(px(1.0))
            .font_family("monospace")
            .text_xs();

        if self.patch.is_empty() {
            return container.child(
                div()
                    .text_color(theme.colors.muted_foreground)
                    .child("No diff changes."),
            );
        }

        for line in self.patch.lines() {
            let (bg, fg) = if line.starts_with('+') && !line.starts_with("+++") {
                (theme.colors.success.opacity(0.12), theme.colors.success)
            } else if line.starts_with('-') && !line.starts_with("---") {
                (theme.colors.danger.opacity(0.12), theme.colors.danger)
            } else if line.starts_with("@@") {
                (theme.colors.primary.opacity(0.12), theme.colors.primary)
            } else {
                (
                    theme.colors.group_box.opacity(0.0),
                    theme.colors.group_box_foreground,
                )
            };

            container = container.child(
                div()
                    .min_w_full()
                    .whitespace_nowrap()
                    .px_1()
                    .py(px(1.0))
                    .bg(bg)
                    .text_color(fg)
                    .child(line.to_string()),
            );
        }

        if self.truncated {
            container = container.child(
                div()
                    .w_full()
                    .p_2()
                    .bg(theme.colors.warning.opacity(0.15))
                    .text_color(theme.colors.warning)
                    .child("... (Diff truncated due to maximum byte limit)"),
            );
        }

        container
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_view_initializes_with_patch_and_truncation() {
        let view = DiffView::new("+ added line\n- removed line", true);
        assert_eq!(view.patch, "+ added line\n- removed line");
        assert!(view.truncated);
    }
}
