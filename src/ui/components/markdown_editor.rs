use std::rc::Rc;

use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, IntoElement, Render, RenderOnce, Subscription, Window, div, px,
};

pub type MarkdownChangeCallback = Rc<dyn Fn(String, &mut App) + 'static>;

struct MarkdownEditorHolder {
    editor: Entity<EditorState>,
    last_synced_text: String,
    on_change: Option<MarkdownChangeCallback>,
    _subscription: Subscription,
}

impl Render for MarkdownEditorHolder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct MarkdownEditor {
    text: String,
    placeholder: String,
    on_change: Option<MarkdownChangeCallback>,
}

impl MarkdownEditor {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            placeholder: "Type release notes in Markdown, or generate a draft above...".to_string(),
            on_change: None,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(String, &mut App) + 'static,
    {
        self.on_change = Some(Rc::new(callback));
        self
    }
}

impl RenderOnce for MarkdownEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial_text = self.text.clone();
        let initial_placeholder = self.placeholder.clone();
        let initial_on_change = self.on_change.clone();

        let holder_state = window.use_state(cx, move |window, cx| {
            let editor = cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("markdown")
                    .line_number(true)
                    .placeholder(initial_placeholder)
                    .default_value(initial_text.clone())
            });

            let subscription = cx.subscribe(
                &editor,
                |holder: &mut MarkdownEditorHolder, editor, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        let current = editor.read(cx).value().to_string();
                        holder.last_synced_text = current.clone();
                        if let Some(cb) = holder.on_change.clone() {
                            cb(current, cx);
                        }
                    }
                },
            );

            MarkdownEditorHolder {
                editor,
                last_synced_text: initial_text,
                on_change: initial_on_change,
                _subscription: subscription,
            }
        });

        let target_text = self.text;
        let updated_cb = self.on_change;
        let editor_entity = holder_state.update(cx, |holder, cx| {
            holder.on_change = updated_cb;
            if holder.last_synced_text != target_text {
                holder.last_synced_text = target_text.clone();
                let text_to_set = target_text;
                holder.editor.update(cx, |ed, cx| {
                    ed.set_value(text_to_set, window, cx);
                });
            }
            holder.editor.clone()
        });

        div()
            .id("markdown_editor_container")
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(360.0))
            .child(Editor::new(&editor_entity).w_full().h_full())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_editor_holds_text() {
        let editor = MarkdownEditor::new("# Header\nContent line")
            .placeholder("Custom placeholder")
            .on_change(|_, _| {});
        assert_eq!(editor.text, "# Header\nContent line");
        assert_eq!(editor.placeholder, "Custom placeholder");
        assert!(editor.on_change.is_some());
    }
}
