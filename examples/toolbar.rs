//! The themed editor with its optional formatting toolbar.
//! `cargo run --example toolbar`; Cmd/Ctrl+Shift+T hides or shows it.
use gpui_kit::*;
use markdown_editor::{BlockEditorState, ToggleToolbar, component::BlockEditor};

struct Demo {
    editor: Entity<BlockEditorState>,
}

impl Render for Demo {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            BlockEditor::new(&self.editor)
                .auto_grow(8, 30)
                .toolbar(true),
        )
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            markdown_editor::init(cx);
            // Opt-in shortcut; the editor binds nothing for the toolbar itself.
            let keys = if cfg!(target_os = "macos") {
                "cmd-shift-t"
            } else {
                "ctrl-shift-t"
            };
            cx.bind_keys([KeyBinding::new(keys, ToggleToolbar, Some("BlockEditor"))]);
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Demo {
                    editor: cx.new(|cx| {
                        BlockEditorState::new(window, cx).with_source(
                            "# Toolbar\n\nSelect text and use **Bold**, *Italic* or `code`.",
                        )
                    }),
                })
            })
            .expect("failed to open window");
        });
}
