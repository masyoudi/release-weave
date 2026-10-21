use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, IntoElement, RenderOnce, Window, div, px};

#[derive(IntoElement)]
pub struct MarkdownPreview {
    content: String,
}

impl MarkdownPreview {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl RenderOnce for MarkdownPreview {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let mut container = v_flex()
            .w_full()
            .p_4()
            .rounded_md()
            .bg(theme.colors.group_box)
            .border_1()
            .border_color(theme.colors.border)
            .gap_2()
            .text_sm()
            .text_color(theme.colors.group_box_foreground);

        if self.content.trim().is_empty() {
            return container.child(
                div()
                    .text_color(theme.colors.muted_foreground)
                    .child("No content to display."),
            );
        }

        for line in self.content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("# ") {
                container = container.child(
                    div()
                        .text_xl()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(theme.colors.foreground)
                        .child(trimmed.trim_start_matches("# ").to_string()),
                );
            } else if trimmed.starts_with("## ") {
                container = container.child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(theme.colors.foreground)
                        .child(trimmed.trim_start_matches("## ").to_string()),
                );
            } else if trimmed.starts_with("### ") {
                container = container.child(
                    div()
                        .text_base()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(theme.colors.foreground)
                        .child(trimmed.trim_start_matches("### ").to_string()),
                );
            } else if trimmed.starts_with("- ") {
                container = container.child(
                    h_flex()
                        .items_start()
                        .gap_2()
                        .child(div().text_color(theme.colors.primary).child("•"))
                        .child(div().child(trimmed.trim_start_matches("- ").to_string())),
                );
            } else if trimmed.is_empty() {
                container = container.child(div().h(px(6.0)));
            } else {
                container = container.child(div().child(trimmed.to_string()));
            }
        }

        container
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_preview_holds_content() {
        let prev = MarkdownPreview::new("# Title\n- Item 1\n- Item 2");
        assert_eq!(prev.content, "# Title\n- Item 1\n- Item 2");
    }
}
