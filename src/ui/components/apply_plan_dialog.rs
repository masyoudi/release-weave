use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClickEvent, IntoElement, RenderOnce, Window, div, px};

use crate::domain::file_change::FileChangePlan;
use crate::ui::components::diff_view::DiffView;

pub type DialogActionCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
pub type PatchSelectCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct ApplyPlanDialog {
    plan: FileChangePlan,
    selected_idx: usize,
    on_select_patch: Option<PatchSelectCallback>,
    on_confirm: DialogActionCallback,
    on_cancel: DialogActionCallback,
}

impl ApplyPlanDialog {
    pub fn new<FConfirm, FCancel>(
        plan: FileChangePlan,
        selected_idx: usize,
        on_confirm: FConfirm,
        on_cancel: FCancel,
    ) -> Self
    where
        FConfirm: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        FCancel: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        Self {
            plan,
            selected_idx,
            on_select_patch: None,
            on_confirm: Rc::new(on_confirm),
            on_cancel: Rc::new(on_cancel),
        }
    }

    pub fn on_select_patch<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_select_patch = Some(Rc::new(callback));
        self
    }
}

impl RenderOnce for ApplyPlanDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let cancel_fn = self.on_cancel.clone();
        let confirm_fn = self.on_confirm.clone();

        let current_patch = self
            .plan
            .patches
            .get(self.selected_idx)
            .or_else(|| self.plan.patches.first());
        let current_diff = current_patch
            .map(|p| p.to_unified_diff())
            .unwrap_or_else(|| "No file diff available.".to_string());

        v_flex()
            .id("apply_plan_backdrop")
            .absolute()
            .inset_0()
            .size_full()
            .items_center()
            .justify_center()
            .bg(theme.overlay)
            .child(
                v_flex()
                    .id("apply_plan_card")
                    .w(px(800.0))
                    .max_h(px(640.0))
                    .rounded_xl()
                    .bg(theme.colors.group_box)
                    .border_1()
                    .border_color(theme.colors.border)
                    .shadow_2xl()
                    .p_5()
                    .gap_4()
                    .child(
                        // Header
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_start()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                                    .text_color(theme.colors.foreground)
                                                    .child(format!("Apply Release v{} to Files", self.plan.target_version)),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme.colors.primary.opacity(0.12))
                                                    .text_color(theme.colors.primary)
                                                    .text_xs()
                                                    .child(format!("{} file(s) to update", self.plan.patches.len())),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.muted_foreground)
                                            .child("Review unified diffs before applying target version and changelog entry to disk."),
                                    ),
                            ),
                    )
                    .child(
                        // Split Body: File list on left, DiffView on right
                        h_flex()
                            .w_full()
                            .h(px(400.0))
                            .gap_4()
                            .child(
                                // Left pane: list of files
                                v_flex()
                                    .id("apply_plan_files_scroll")
                                    .w(px(240.0))
                                    .h_full()
                                    .overflow_y_scrollbar()
                                    .border_1()
                                    .border_color(theme.colors.border)
                                    .rounded_md()
                                    .p_2()
                                    .gap_1p5()
                                    .children(self.plan.patches.iter().enumerate().map(|(idx, patch)| {
                                        let is_selected = idx == self.selected_idx;
                                        let select_callback = self.on_select_patch.clone();

                                        let mut item_box = h_flex()
                                            .w_full()
                                            .px_2p5()
                                            .py_2()
                                            .rounded_md()
                                            .justify_between()
                                            .items_center()
                                            .cursor_pointer();

                                        if is_selected {
                                            item_box = item_box
                                                .bg(theme.colors.primary.opacity(0.12))
                                                .border_1()
                                                .border_color(theme.colors.primary);
                                        } else {
                                            item_box = item_box
                                                .bg(theme.colors.muted.opacity(0.3))
                                                .border_1()
                                                .border_color(theme.colors.border);
                                        }

                                        item_box
                                            .on_mouse_down(gpui_kit::MouseButton::Left, move |_, window, cx| {
                                                if let Some(cb) = &select_callback {
                                                    cb(idx, window, cx);
                                                }
                                            })
                                            .child(
                                                v_flex()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                            .text_color(if is_selected {
                                                                theme.colors.primary
                                                            } else {
                                                                theme.colors.foreground
                                                            })
                                                            .child(patch.relative_path.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.colors.muted_foreground)
                                                            .child(patch.diff_summary()),
                                                    ),
                                            )
                                    })),
                            )
                            .child(
                                // Right pane: DiffView
                                v_flex()
                                    .id("apply_plan_diff_scroll")
                                    .flex_1()
                                    .h_full()
                                    .min_w_0()
                                    .overflow_y_scrollbar()
                                    .child(DiffView::new(current_diff, false)),
                            ),
                    )
                    .child(
                        // Footer Actions
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme.colors.border)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child("Changes are checked against disk hash and applied atomically."),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("btn_cancel_apply")
                                            .label("Cancel")
                                            .small()
                                            .ghost()
                                            .on_click(move |e, window, cx| {
                                                cancel_fn(e, window, cx);
                                            }),
                                    )
                                    .child(
                                        Button::new("btn_confirm_apply")
                                            .icon(IconName::Check)
                                            .label("Apply Changes")
                                            .small()
                                            .primary()
                                            .on_click(move |e, window, cx| {
                                                confirm_fn(e, window, cx);
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}
