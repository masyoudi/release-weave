use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, IntoElement, RenderOnce, Window, div, px};

use crate::domain::check::{CheckResult, CheckStatus};

pub fn check_status_symbol(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Passed => "✓",
        CheckStatus::Failed => "✗",
        CheckStatus::Warning => "⚠",
        CheckStatus::Skipped => "⊘",
        CheckStatus::NotRun => "-",
    }
}

pub fn check_status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Passed => "Passed",
        CheckStatus::Failed => "Failed",
        CheckStatus::Warning => "Warning",
        CheckStatus::Skipped => "Skipped",
        CheckStatus::NotRun => "Not run",
    }
}

pub fn check_status_accessible_text(status: CheckStatus) -> String {
    format!(
        "{} {}",
        check_status_symbol(status),
        check_status_label(status)
    )
}

#[derive(IntoElement)]
pub struct CheckStatusBadge {
    status: CheckStatus,
}

impl CheckStatusBadge {
    pub fn new(status: CheckStatus) -> Self {
        Self { status }
    }
}

impl RenderOnce for CheckStatusBadge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let label = check_status_label(self.status);
        let icon_name = match self.status {
            CheckStatus::Passed => IconName::CircleCheck,
            CheckStatus::Failed => IconName::CircleX,
            CheckStatus::Warning => IconName::TriangleAlert,
            CheckStatus::Skipped => IconName::Ban,
            CheckStatus::NotRun => IconName::Minus,
        };

        let (bg, fg, border) = match self.status {
            CheckStatus::Passed => (
                theme.colors.success.opacity(0.15),
                theme.colors.success,
                theme.colors.success.opacity(0.4),
            ),
            CheckStatus::Failed => (
                theme.colors.danger.opacity(0.15),
                theme.colors.danger,
                theme.colors.danger.opacity(0.4),
            ),
            CheckStatus::Warning => (
                theme.colors.warning.opacity(0.15),
                theme.colors.warning,
                theme.colors.warning.opacity(0.4),
            ),
            CheckStatus::Skipped => (
                theme.colors.secondary.opacity(0.5),
                theme.colors.secondary_foreground,
                theme.colors.border,
            ),
            CheckStatus::NotRun => (
                theme.colors.muted.opacity(0.5),
                theme.colors.muted_foreground,
                theme.colors.border,
            ),
        };

        h_flex()
            .items_center()
            .gap_1()
            .px_2()
            .py(px(2.0))
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(fg)
            .text_xs()
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .child(Icon::new(icon_name).text_color(fg))
            .child(div().child(label))
    }
}

#[derive(IntoElement)]
pub struct CheckListItem {
    result: CheckResult,
}

impl CheckListItem {
    pub fn new(result: CheckResult) -> Self {
        Self { result }
    }
}

impl RenderOnce for CheckListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let check = self.result;

        let required_text = if check.required {
            "[Required]"
        } else {
            "[Optional]"
        };

        v_flex()
            .w_full()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.group_box)
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.group_box_foreground)
                                    .child(check.check_type.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(required_text),
                            ),
                    )
                    .child(CheckStatusBadge::new(check.status)),
            )
            .when_some(check.details, |this, details| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.muted_foreground)
                        .child(details),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_five_check_statuses_have_unique_accessible_labels_without_relying_on_color() {
        let statuses = [
            CheckStatus::Passed,
            CheckStatus::Failed,
            CheckStatus::Warning,
            CheckStatus::Skipped,
            CheckStatus::NotRun,
        ];

        let mut symbols = HashSet::new();
        let mut labels = HashSet::new();
        let mut full_texts = HashSet::new();

        for s in statuses {
            let sym = check_status_symbol(s);
            let lbl = check_status_label(s);
            let full = check_status_accessible_text(s);

            assert!(!sym.is_empty());
            assert!(!lbl.is_empty());

            symbols.insert(sym);
            labels.insert(lbl);
            full_texts.insert(full);
        }

        assert_eq!(symbols.len(), 5);
        assert_eq!(labels.len(), 5);
        assert_eq!(full_texts.len(), 5);
    }
}
