//! Optional formatting toolbar for [`crate::component::BlockEditor`].
//!
//! Composed only from published gpui-kit parts: a `Toolbar` (roving arrow-key
//! focus, toolbar semantics), ghost `Toggle`s for the three inline marks, and
//! ghost `Button` + `DropdownMenu` triggers for paragraph style and insertion.
//! Every control routes through [`BlockEditorState::run`] and hands focus back
//! to the editor, so the caret and selection survive a pointer click.
use gpui::{App, Entity, IntoElement, RenderOnce, Styled, Window, div, rems};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _, Toggle, ToggleVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
    separator::Separator,
    toolbar::Toolbar,
};

use crate::{BlockEditorState, BlockKind, EditorCommand, MarkKind, MarkState};

// Lucide icons (ISC, see assets/icons/LICENSE-LUCIDE), copied from the
// gpui-kit-assets 0.9 catalog and embedded as bytes (`Icon::data`) so apps
// need no asset-source wiring for the toolbar.
const ICON_BOLD: &[u8] = include_bytes!("../assets/icons/bold.svg");
const ICON_ITALIC: &[u8] = include_bytes!("../assets/icons/italic.svg");
const ICON_STRIKETHROUGH: &[u8] = include_bytes!("../assets/icons/strikethrough.svg");
const ICON_CODE: &[u8] = include_bytes!("../assets/icons/code.svg");
const ICON_QUOTE: &[u8] = include_bytes!("../assets/icons/text-quote.svg");
const ICON_INDENT: &[u8] = include_bytes!("../assets/icons/list-indent-increase.svg");
const ICON_OUTDENT: &[u8] = include_bytes!("../assets/icons/list-indent-decrease.svg");

fn icon(svg: &[u8]) -> Icon {
    Icon::default().data(svg)
}

/// Whether the caret is in a table cell (quotes wrap whole blocks, not cells).
fn in_table_cell(model: &crate::BlockEditorModel) -> bool {
    model.caret().is_some_and(|c| {
        model
            .document()
            .block(c.block)
            .and_then(|b| b.parent())
            .is_some_and(|p| p.kind() == BlockKind::TableRow)
    })
}

/// Style trigger width in rems: the longest label plus caret and padding.
const STYLE_TRIGGER_WIDTH: f32 = 7.5;

/// Paragraph styles offered by the style dropdown, in menu order.
const STYLES: &[(&str, BlockKind)] = &[
    ("Normal text", BlockKind::Paragraph),
    ("Heading 1", BlockKind::Heading { level: 1 }),
    ("Heading 2", BlockKind::Heading { level: 2 }),
    ("Heading 3", BlockKind::Heading { level: 3 }),
    ("Heading 4", BlockKind::Heading { level: 4 }),
];

/// Label for the caret's text-block kind, as shown on the style trigger.
pub(crate) fn style_label(kind: Option<BlockKind>) -> &'static str {
    match kind {
        Some(BlockKind::CodeBlock) => "Code block",
        Some(BlockKind::Heading { level: 5 }) => "Heading 5",
        Some(BlockKind::Heading { level: 6 }) => "Heading 6",
        Some(kind) => STYLES
            .iter()
            .find(|(_, k)| *k == kind)
            .map_or("Normal text", |(label, _)| label),
        None => "Normal text",
    }
}

fn shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{}", key.to_uppercase())
    } else {
        format!("Ctrl+{}", key.to_uppercase())
    }
}

/// Run `command` on `state`, then return focus to the editor.
pub(crate) fn run(
    state: &Entity<BlockEditorState>,
    command: EditorCommand,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, cx| {
        let _ = state.run(command, cx);
        state.focus(window, cx);
    });
}

/// A compact, conventional WYSIWYG toolbar bound to one editor.
///
/// [`crate::component::BlockEditor::toolbar`] places it above the editor; use
/// `EditorToolbar` directly to host it elsewhere (a title bar or pane header).
/// It renders nothing while [`BlockEditorState::is_toolbar_hidden`] is set.
#[derive(IntoElement)]
pub struct EditorToolbar {
    state: Entity<BlockEditorState>,
}

impl EditorToolbar {
    pub fn new(state: &Entity<BlockEditorState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for EditorToolbar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let editor = self.state.read(cx);
        if editor.is_toolbar_hidden() {
            return div().into_any_element();
        }
        let id = self.state.entity_id();
        let readonly = editor.is_readonly();
        let model = editor.model();
        let kind = model.block_kind_at_caret();
        let in_text = kind.is_some();
        let mark_on = |mark| model.mark_state(mark) == MarkState::On;
        let (bold, italic, strike, code) = (
            mark_on(MarkKind::Bold),
            mark_on(MarkKind::Italic),
            mark_on(MarkKind::Strikethrough),
            mark_on(MarkKind::Code),
        );
        let in_code_block = kind == Some(BlockKind::CodeBlock);
        let in_quote = model.in_blockquote();
        let quotable = in_text && !in_table_cell(model);
        let (can_indent, can_outdent) = (model.can_indent(), model.can_outdent());
        let marks_disabled = readonly || !in_text || in_code_block;

        let state = self.state.clone();
        let style = Button::new(("md-toolbar-style", id))
            .ghost()
            .small()
            .label(style_label(kind))
            .dropdown_caret(true)
            // Fixed width (fits "Normal text" / "Heading 6" + caret) so
            // changing style never shifts the rest of the bar.
            .w(rems(STYLE_TRIGGER_WIDTH))
            .tooltip("Paragraph style")
            .disabled(readonly || !in_text)
            .dropdown_menu(move |mut menu, _, _| {
                for (label, target) in STYLES {
                    let state = state.clone();
                    let target = *target;
                    menu = menu.item(
                        PopupMenuItem::new(*label)
                            .checked(kind == Some(target))
                            .on_click(move |_, window, cx| {
                                run(&state, EditorCommand::SetBlockKind(target), window, cx)
                            }),
                    );
                }
                menu
            });

        let mark = |name: &'static str, tip: String, mark: MarkKind, on: bool, svg: &[u8]| {
            let state = self.state.clone();
            Toggle::new((name, id))
                .ghost()
                .checked(on)
                .tooltip(tip)
                .disabled(marks_disabled)
                .icon(icon(svg))
                .on_click(move |_, window, cx| {
                    run(&state, EditorCommand::ToggleMark(mark), window, cx)
                })
        };
        let bold = mark(
            "md-toolbar-bold",
            format!("Bold ({})", shortcut("b")),
            MarkKind::Bold,
            bold,
            ICON_BOLD,
        );
        let italic = mark(
            "md-toolbar-italic",
            format!("Italic ({})", shortcut("i")),
            MarkKind::Italic,
            italic,
            ICON_ITALIC,
        );
        let strike = mark(
            "md-toolbar-strike",
            "Strikethrough".into(),
            MarkKind::Strikethrough,
            strike,
            ICON_STRIKETHROUGH,
        );
        let code = mark(
            "md-toolbar-code",
            format!("Inline code ({})", shortcut("e")),
            MarkKind::Code,
            code,
            ICON_CODE,
        );

        // Block structure: quote toggle, then list outdent / indent. Always
        // shown (disabled when they would do nothing) so the bar never
        // reflows as the caret moves.
        let state = self.state.clone();
        let quote = Toggle::new(("md-toolbar-quote", id))
            .ghost()
            .checked(in_quote)
            .tooltip("Quote")
            .disabled(readonly || !(in_quote || quotable))
            .icon(icon(ICON_QUOTE))
            .on_click(move |_, window, cx| {
                run(&state, EditorCommand::ToggleBlockquote, window, cx)
            });
        let step = |name: &'static str,
                    tip: &'static str,
                    svg: &[u8],
                    enabled: bool,
                    command: EditorCommand| {
            let state = self.state.clone();
            Button::new((name, id))
                .ghost()
                .icon(icon(svg))
                .tooltip(tip)
                .disabled(readonly || !enabled)
                .on_click(move |_, window, cx| run(&state, command.clone(), window, cx))
        };
        let outdent = step(
            "md-toolbar-outdent",
            "Decrease list indent (Shift+Tab)",
            ICON_OUTDENT,
            can_outdent,
            EditorCommand::Outdent,
        );
        let indent = step(
            "md-toolbar-indent",
            "Increase list indent (Tab)",
            ICON_INDENT,
            can_indent,
            EditorCommand::Indent,
        );

        let state = self.state.clone();
        let insert = Button::new(("md-toolbar-insert", id))
            .ghost()
            .small()
            .label("Insert")
            .dropdown_caret(true)
            .tooltip("Insert")
            .disabled(readonly)
            .dropdown_menu(move |menu, _, _| {
                let item = |label: &'static str, command: EditorCommand| {
                    let state = state.clone();
                    PopupMenuItem::new(label)
                        .on_click(move |_, window, cx| run(&state, command.clone(), window, cx))
                };
                menu.item(item(
                    "Table",
                    EditorCommand::InsertTable { rows: 3, cols: 3 },
                ))
                .item(item(
                    "Code block",
                    EditorCommand::SetCodeBlock { lang: None },
                ))
                .item(item(
                    "Horizontal rule",
                    EditorCommand::InsertBlock(BlockKind::HorizontalRule),
                ))
                .separator()
                .item(item(
                    "Bulleted list",
                    EditorCommand::ToggleList { ordered: false },
                ))
                .item(item(
                    "Numbered list",
                    EditorCommand::ToggleList { ordered: true },
                ))
                .item(item("Task list", EditorCommand::ToggleTask))
            });

        Toolbar::new(("md-toolbar", id))
            .content(style)
            .content(Separator::vertical().h_4())
            .child(bold)
            .child(italic)
            .child(strike)
            .child(code)
            .content(Separator::vertical().h_4())
            .child(quote)
            .child(outdent)
            .child(indent)
            .content(Separator::vertical().h_4())
            .content(insert)
            .into_any_element()
    }
}
