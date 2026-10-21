# macOS manual test checklist (about 15 minutes)

For a real Mac; CI runs on Linux and cannot press macOS-only bindings. Run the
story with a scratch document:

```sh
cat > /tmp/qa.md <<'MD'
# Heading one

alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega

Plain **bold** and *italic* and `code` and [a link](https://example.com).

- first item
- second item

> quoted text here

```rust
fn main() {
    let x = 1;
}
```

| Name | Qty | Note |
| :--- | :---: | ---: |
| apple | 3 | red |
| pear | 5 | green |
MD
STORY_MD=/tmp/qa.md cargo run --example story
```

Reload the story (quit and rerun) whenever a section says "fresh document".
Tick each box; note anything that differs from **Expected**.

## 1. Line and word keys (D21), about 4 min

| # | Steps | Expected |
| --- | --- | --- |
| 1.1 | Click after "kappa" in the long paragraph (make the window narrow enough that it wraps to 2+ lines). Press Cmd+Backspace. | Only the text from the start of that *visual* line up to the caret goes; other lines stay. |
| 1.2 | Cmd+Z. | The text is back; caret where it was. |
| 1.3 | Caret mid-line, Cmd+Delete (fn+Cmd+Backspace). | Text from the caret to the end of that visual line goes. Cmd+Z restores. |
| 1.4 | Caret after "gamma". Option+Backspace three times, then Cmd+Z once. | "gamma", "beta", "alpha " go one word at a time; one Cmd+Z restores all three. |
| 1.5 | Caret before "beta". Option+Delete (fn+Option+Backspace). | "beta" goes (word forward). |
| 1.6 | Option+Backspace twice, then Backspace once, then Cmd+Z. | Cmd+Z undoes only the plain Backspace; a second Cmd+Z undoes both word deletes together. |
| 1.7 | In "Plain **bold** and", put the caret right after "bold"; Option+Backspace. | "bold" goes; "Plain " and " and" keep their formatting. |
| 1.8 | Select a few characters, press each of Cmd+Backspace, Option+Backspace, Option+Delete, Cmd+Delete (undo between). | Each deletes exactly the selection, nothing more. |
| 1.9 | Caret at the very start of "Plain..." paragraph; Cmd+Backspace, then Cmd+Z; Option+Backspace, then Cmd+Z. | Both act like Backspace: the paragraph joins the long paragraph above. |
| 1.10 | Caret at the start of "Heading one"; Option+Backspace. | The heading becomes a paragraph (like Backspace). |
| 1.11 | Caret at the start of "second item"; Cmd+Backspace. | Same as Backspace at that point (join into the first item). |
| 1.12 | Caret at the start of "quoted text here"; Option+Backspace. | Same as Backspace: the paragraph lifts out of the quote. |
| 1.13 | In the code block, caret at the end of `    let x = 1;`; Cmd+Backspace. | Only that code line's text goes (the line stays, empty). Press Cmd+Backspace again: the newline goes (lines join). Never leaves the code block. |
| 1.14 | Ctrl+A, Ctrl+E in a paragraph; Ctrl+Shift+A / Ctrl+Shift+E. | Caret to line start / end; with Shift, selects to there. |
| 1.15 | Ctrl+D and Ctrl+H mid-word. | Delete forward / backward one character. |
| 1.16 | Ctrl+K mid-line. | Deletes to the end of the line. (There is no paste-back: Ctrl+Y does nothing special.) |
| 1.17 | Cmd+Left, Cmd+Right, Cmd+Shift+Left, Cmd+Shift+Right in the wrapped paragraph. | Start / end of the visual line; Shift selects. |
| 1.18 | Option+Up, Option+Up again; Option+Down twice; Option+Shift+Down. | Start of the block, then start of the previous block; end of the block, then end of the next; Shift selects. |
| 1.19 | Cmd+Up, Cmd+Down. | Document start / end (unchanged). |

## 2. Tables (D20) and normal text, about 4 min (fresh document)

| # | Steps | Expected |
| --- | --- | --- |
| 2.1 | Caret in "apple". Option+Backspace repeatedly, then Option+Delete. | The cell empties; further presses do nothing (cells never merge, caret stays). |
| 2.2 | Caret at the start of "3" (second column); Cmd+Backspace. Caret at the end of "red"; Cmd+Delete. | Nothing happens in either. |
| 2.3 | Caret in a cell; Cmd+Option+Right, Cmd+Option+Left. | A column is inserted right / left; caret goes to the new cell. Cmd+Z removes each. |
| 2.4 | Cmd+Option+Down, Cmd+Option+Up. | A row is inserted below / above. |
| 2.5 | Caret in a body row; Cmd+Option+Backspace. Then caret in the header row; Cmd+Option+Backspace. | The body row is deleted; in the header row nothing happens. |
| 2.6 | Hover the table. | A thin grip shows above the hovered column and left of the hovered row; the pointer is an open hand over a grip. |
| 2.7 | Click a column grip; choose Centre, then Right, then None from the alignment row. | The column re-aligns each time; the menu stays open; the caret and clicks in those cells land on the text. |
| 2.8 | Click a row grip in the header row. | Delete row is disabled ("The header row can't be deleted"). Escape closes the menu. |
| 2.9 | Caret in "Heading one"; Cmd+Option+0. Select across two blocks; Cmd+Option+0. | The heading becomes a paragraph; each selected block becomes a paragraph. |

## 3. IME, about 3 min (fresh document)

| # | Steps | Expected |
| --- | --- | --- |
| 3.1 | US or ABC keyboard: in a paragraph type Option+E then E, Option+U then U, Option+N then N. | "é", "ü", "ñ" appear; the dead key shows underlined while pending. Cmd+Z removes each character as one step. |
| 3.2 | Hold "e" and pick "è" from the accent popup. | "è" replaces the "e". |
| 3.3 | Japanese (Romaji) input: type `nihongo`, press Space to convert, Enter to commit. | Underlined composition while typing; candidate window next to the caret; Enter commits "日本語" without a new line. One Cmd+Z removes it. |
| 3.4 | During a Japanese composition, press Option+Backspace and Cmd+Backspace. | The IME handles them (edits the composition) or the composition ends first; no stray characters, no crash. |
| 3.5 | Pinyin (Simplified Chinese): type `zhongwen`, pick a candidate with 1. | "中文" is committed. Repeat inside a table cell and in a heading. |
| 3.6 | Start a composition inside **bold** text and commit. | The committed text is bold. |

## 4. Copy and paste, about 2 min (fresh document)

| # | Steps | Expected |
| --- | --- | --- |
| 4.1 | Select "**bold** and *italic*" in one paragraph; Cmd+C; paste into the long paragraph. | Bold and italic come across. |
| 4.2 | Select from mid-heading to mid-list; Cmd+C; paste at the end. | Heading, paragraphs and list structure come across (D18). |
| 4.3 | Drag-select two of the three table columns (cells highlight); Cmd+C; paste into an empty paragraph at the end. | A two-column table with just those cells. |
| 4.4 | Cmd+C a link; Cmd+Shift+V into a paragraph. | Plain text, no link. |
| 4.5 | Copy a paragraph with marks; paste into TextEdit (rich text off). | Markdown text (`**bold**` etc.). |

## 5. Drag to reorder (D22), about 2 min (fresh document)

| # | Steps | Expected |
| --- | --- | --- |
| 5.1 | Hover left of "Plain..." paragraph in the gutter. | A thin grip appears. |
| 5.2 | Drag the grip down over the list. | The paragraph is outlined; a horizontal line shows the drop gap; release moves it there. Cmd+Z puts it back in one step. |
| 5.3 | Drag the list by the grip left of an item. | The whole list moves; items cannot be dragged on their own. |
| 5.4 | Drag the table by the strip left of its first column, to the top. | The whole table moves. |
| 5.5 | Click (no drag) the gutter left of a paragraph. | The caret goes to the line start; nothing moves. |
