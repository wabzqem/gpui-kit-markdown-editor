use gpui_kit::*;
use markdown_editor::{BlockEditor, BlockEditorEvent, BlockEditorState};

struct Notes {
    editor: Entity<BlockEditorState>,
    scroll: ScrollHandle,
}

impl Notes {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scroll = ScrollHandle::new();
        let editor = cx.new(|cx| {
            let mut state =
                BlockEditorState::new(window, cx).with_source("# Hello\n\nType **here**.");
            // Optional: keep the caret in view inside your scroll container.
            state.set_scroll_handle(scroll.clone());
            state
        });
        cx.subscribe(&editor, |_, editor, event: &BlockEditorEvent, cx| {
            if let BlockEditorEvent::Changed { .. } = event {
                let _markdown = editor.read(cx).source();
            }
        })
        .detach();
        Self { editor, scroll }
    }
}

impl Render for Notes {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("notes")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            // `min_h_full` makes the whole window clickable, not only the text.
            .child(BlockEditor::new(&self.editor).min_h_full().p_4())
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        markdown_editor::init(cx); // key bindings; call after gpui_kit::init
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Notes::new(window, cx))
        })
        .expect("failed to open window");
    });
}
