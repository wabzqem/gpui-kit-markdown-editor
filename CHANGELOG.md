# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate uses
[Semantic Versioning](https://semver.org/). Design decisions (D1-D22) are in
[`docs/design.md`](docs/design.md).

## [0.1.1] - 2026-10-27

### Fixed
- Pasted URLs keep their exact text in the Markdown source. Bare `http(s)://`
  URLs are written as is, instead of with a backslash before every `_` and
  `&`, which broke them for hosts that send the source on (such as a chat
  composer). A URL that runs into other text is written as `<url>`.
- A link whose text is its own URL is written as an autolink, `<url>`.
- Literal text and link destinations escape less: an underscore between
  letters or digits (`snake_case`) and an `&` that does not start a character
  reference are written without a backslash. Text still parses back to the
  same content.

## [0.1.0] - 2026-10-21

First release. A block-based WYSIWYG Markdown editor for GPUI, rendered
through gpui-kit 0.8's editor host API (`BlockNode` trees in, leaf geometry
out). The library is `markdown_editor`.

### Model and history
- Block tree with process-unique, never-reused block ids, parent links and an
  order index (D1, D4); text, code (`Plain`) and atom leaves.
- Edits are invertible steps grouped into transactions; `EditorCommand`s run
  as one undo step each.
- Undo with intent-based coalescing (typing, Backspace, Delete, word deletes)
  and explicit boundaries on caret moves and blur (D11).
- Selections: text ranges, whole blocks (`BlockSelection::Blocks`) and table
  cell rectangles (`BlockSelection::Cells`).

### Markdown
- Import and lossless-in-meaning export of CommonMark/GFM: headings,
  paragraphs, bullet, ordered and task lists, quotes, fenced code, tables
  with column alignment, images and other inline atoms, links, and marks
  (bold, italic, strikethrough, underline, inline code, highlight as `<mark>`)
  (D5).
- Markdown the model does not represent stays literal and is escaped on save
  (D15).
- Input rules as a registry with Markdown defaults: `# `, `- `, `1. `,
  `[ ] `, `> `, code fences, and inline `` `x` ``, `**x**`, `*x*`, `~~x~~`,
  each its own undo step (D13).
- Mark boundary behaviour: bold, italic, strikethrough, underline and
  highlight extend at their trailing edge; links and inline code do not (D6).
- Public mark API: `MarkKind`, `MarkSet`, `MarkChange` (D12).

### Editing
- Code blocks edited in place, syntax highlighted (`syntect` by default,
  `tree-sitter` optional) (D7).
- Links and tasks: Cmd/Ctrl+K asks the host to edit the link
  (`EditLinkRequested`, then `set_link`), hovering reports the URL,
  Cmd/Ctrl+click emits `LinkClicked` (the editor never opens a browser), and
  task checkboxes toggle on click (D14).
- Leaving containers works like leaving an empty list item: Enter on an empty
  quote paragraph leaves or splits the quote, Backspace at a quote's start
  lifts the paragraph out, and an empty trailing paragraph in a list item
  becomes an item of its own; one undo step each (D16).
- Headings: Backspace at the start, emptying, or Enter in an empty heading
  gives a paragraph; Cmd/Ctrl+Alt+0 makes any block (or every selected block)
  normal text (D19).
- Tables without buttons: hover grips above columns and beside rows open a
  menu (insert, delete, column alignment); Cmd/Ctrl+Alt+arrows insert
  columns and rows, Cmd/Ctrl+Alt+Backspace deletes; caret, clicks and grips
  follow cell alignment (D20).
- Platform text-editing keys: Option/Ctrl+Backspace and Delete delete words;
  on macOS Cmd+Backspace/Delete delete to the visual line edge, Ctrl+K,
  Ctrl+A/E, Ctrl+D/H, Cmd+Left/Right; Option+Up/Down (macOS) and Ctrl+Up/Down
  (elsewhere) move by paragraph. Deletes stay inside table cells and code
  lines; word deletes coalesce only with each other (D21).
- Drag to reorder tables and code blocks by their edge handle, with a drop
  indicator; one undo step (D22).

### Clipboard
- Copy writes inline marks as Markdown, tagged so the editor's own paste
  parses it; external text uses a conservative import; Cmd/Ctrl+Shift+V pastes
  plain text (D17).
- Copying across several blocks keeps the whole structure (headings, lists,
  quotes, code, tables); a cell rectangle copies as a Markdown table of just
  those cells (D18).

### Rendering, host and platform
- Direct model-to-`BlockNode` projection with a per-node revision cache,
  light culling of off-screen units, and deterministic per-frame counters
  tested in CI (D3, D9).
- IME composition with underline, one undo step per composition.
- Accessibility: multiline text input role with a lazily built value (D8).
- Host contract: owner-supplied scroll handle, submit policy for composers,
  clipboard policy, `BlockEditorEvent`s, and `BlockEditorStyle` overrides
  (see `docs/host-contract.md`).
- Naming: `BlockEditorState` / `BlockEditor`, `BlockPosition` /
  `BlockSelection` (D10). Developed as a gpui-kit spike behind a feature
  flag (D2) and extracted into this crate.

### Known limitations
- macOS-only key bindings are not exercised in CI (Linux); see
  `docs/macos-test-checklist.md`.
- Tables and code blocks nested in lists or quotes cannot be dragged.
- Linux desktops may take Ctrl+Alt+arrows; the table menu covers those actions.
