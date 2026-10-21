use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, IntoElement, RenderOnce, Window, div, px};

use crate::domain::provenance::Origin;

#[derive(IntoElement)]
pub struct SourceBadge {
    origin: Origin,
    confidence: Option<f32>,
}

impl SourceBadge {
    pub fn new(origin: Origin) -> Self {
        Self {
            origin,
            confidence: None,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub fn accessible_label(&self) -> String {
        match &self.origin {
            Origin::Deterministic => "[D] Deterministic".to_string(),
            Origin::Ai { provider, model } => match self.confidence {
                Some(conf) => format!("[AI] {provider}:{model} ({:.0}%)", conf * 100.0),
                None => format!("[AI] {provider}:{model}"),
            },
            Origin::External { source } => format!("[EXT] {source}"),
        }
    }
}

impl RenderOnce for SourceBadge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let label = self.accessible_label();

        let (bg, fg, border) = match &self.origin {
            Origin::Deterministic => (
                theme.colors.secondary,
                theme.colors.secondary_foreground,
                theme.colors.border,
            ),
            Origin::Ai { .. } => (
                theme.colors.accent,
                theme.colors.accent_foreground,
                theme.colors.border,
            ),
            Origin::External { .. } => (
                theme.colors.muted,
                theme.colors.muted_foreground,
                theme.colors.border,
            ),
        };

        h_flex()
            .items_center()
            .px_2()
            .py(px(2.0))
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(fg)
            .text_xs()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .child(div().child(label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_accessible_labels_are_distinct() {
        let det = SourceBadge::new(Origin::Deterministic);
        assert_eq!(det.accessible_label(), "[D] Deterministic");

        let ai = SourceBadge::new(Origin::Ai {
            provider: "ollama".into(),
            model: "llama3".into(),
        })
        .with_confidence(0.95);
        assert_eq!(ai.accessible_label(), "[AI] ollama:llama3 (95%)");

        let ext = SourceBadge::new(Origin::External {
            source: "github-pr#42".into(),
        });
        assert_eq!(ext.accessible_label(), "[EXT] github-pr#42");
    }
}
