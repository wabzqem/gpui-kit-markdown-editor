//! The direct projection (`direct.rs`) against `parse(serialize(doc))`.

use super::props::{Rng, document};
use super::*;
use crate::{
    direct::project_block,
    import::{Defs, collect_definitions, import_block},
    step::{DetachedBlock, DetachedContent},
};
use gpui_kit::base::text::editor_host::BlockNode;

/// Structure and content without ids.
fn shape(block: &DetachedBlock, out: &mut String) {
    out.push_str(&format!("{:?}(", block.kind));
    match &block.content {
        DetachedContent::Children(children) => {
            for child in children {
                shape(child, out);
            }
        }
        DetachedContent::Leaf(content) => out.push_str(&format!("{content:?}")),
    }
    out.push(')');
}

fn shapes(nodes: &[BlockNode]) -> String {
    let mut defs = Defs::new();
    collect_definitions(nodes, &mut defs);
    let mut blocks = Vec::new();
    for node in nodes {
        import_block(node, &defs, &mut blocks);
    }
    let mut out = String::new();
    for block in &blocks {
        shape(block, &mut out);
    }
    out
}

/// A paragraph's rendered content: its text plus each mark range (shifted
/// into the paragraph's text; adjacent equal ranges merged so node
/// fragmentation does not matter), images, and custom nodes.
fn paragraph_shape(p: &gpui_kit::base::text::editor_host::Paragraph) -> String {
    use gpui_kit::base::text::editor_host::TextMark;
    let mut text = String::new();
    // The effective mark of every byte, combined with `TextMark::merge` as
    // the renderer does, so overlapping vs. split ranges and node
    // fragmentation compare equal.
    let mut per_byte: Vec<TextMark> = Vec::new();
    let mut extras = Vec::new();
    for node in p.children() {
        let base = text.len();
        text.push_str(node.text());
        per_byte.resize(text.len(), TextMark::default());
        for (range, mark) in node.mark_runs() {
            for m in &mut per_byte[base + range.start..(base + range.end).min(text.len())] {
                m.merge(mark.clone());
            }
        }
        if let Some(image) = node.as_image() {
            extras.push(format!("img@{base}:{image:?}"));
        }
        if let Some(custom) = node.as_custom() {
            extras.push(format!("custom@{base}:{custom:?}"));
        }
    }
    let mut runs: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for (i, mark) in per_byte.iter().enumerate() {
        if *mark == TextMark::default() {
            continue;
        }
        let mark = format!("{mark:?}");
        match runs.last_mut() {
            Some((last, m)) if last.end == i && *m == mark => last.end = i + 1,
            _ => runs.push((i..i + 1, mark)),
        }
    }
    format!("{text:?} {runs:?} {extras:?}")
}

/// Rendered `BlockNode` structure without spans or caches.
fn node_shape(node: &BlockNode, out: &mut String) {
    let list = |children: &[BlockNode], out: &mut String| {
        out.push('[');
        for child in children {
            node_shape(child, out);
        }
        out.push(']');
    };
    match node {
        BlockNode::Root { children, .. } => {
            out.push_str("Root");
            list(children, out);
        }
        BlockNode::Paragraph(p) => out.push_str(&format!("P({})", paragraph_shape(p))),
        BlockNode::Heading {
            level, children, ..
        } => out.push_str(&format!("H{level}({})", paragraph_shape(children))),
        BlockNode::Blockquote { children, .. } => {
            out.push_str("Quote");
            list(children, out);
        }
        BlockNode::List {
            children,
            ordered,
            start,
            ..
        } => {
            out.push_str(&format!("List({ordered},{start:?})"));
            list(children, out);
        }
        BlockNode::ListItem {
            children,
            spread,
            checked,
            ..
        } => {
            out.push_str(&format!("Item({spread},{checked:?})"));
            list(children, out);
        }
        BlockNode::CodeBlock(code) => {
            out.push_str(&format!("Code({:?},{:?})", code.lang(), code.code()))
        }
        BlockNode::Table(table) => {
            out.push_str(&format!("Table({:?})[", table.column_aligns()));
            for row in table.rows() {
                out.push('[');
                for cell in row {
                    out.push_str(&format!("C({})", paragraph_shape(cell)));
                }
                out.push(']');
            }
            out.push(']');
        }
        BlockNode::Break { html, .. } => out.push_str(&format!("Br({html})")),
        BlockNode::HorizontalRule { .. } => out.push_str("Hr"),
        BlockNode::Definition {
            identifier,
            url,
            title,
            ..
        } => out.push_str(&format!("Def({identifier},{url},{title:?})")),
        BlockNode::Custom { node, .. } => out.push_str(&format!("Custom({node:?})")),
        BlockNode::Unknown => out.push_str("Unknown"),
        _ => out.push_str("Other"),
    }
}

fn node_shapes(nodes: &[BlockNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        node_shape(node, &mut out);
    }
    out
}

/// Direct projection equals parsing the serialized document, and every text
/// leaf is keyed (checked through geometry in the view tests).
fn assert_equivalent(m: &BlockEditorModel) {
    let doc = &m.doc;
    let direct: Vec<BlockNode> = doc
        .children(doc.root)
        .iter()
        .filter_map(|id| project_block(doc, *id))
        .collect();
    let source = m.source();
    // The editor importer restores caret trivia that a read-only Markdown
    // renderer intentionally omits. Compare against the primary parser plus
    // that editor contract, not against lossy read-only whitespace handling.
    let reparsed = model(&source);
    let parsed: Vec<BlockNode> = reparsed
        .doc
        .children(reparsed.doc.root)
        .iter()
        .filter_map(|id| project_block(&reparsed.doc, *id))
        .collect();
    assert_eq!(
        shapes(&direct),
        shapes(&parsed),
        "direct projection differs from parse(serialize) for {source:?}"
    );
    // Also compare the rendered node trees themselves, not only after
    // importing them back into the model.
    assert_eq!(
        node_shapes(&direct),
        node_shapes(&parsed),
        "BlockNode trees differ for {source:?}"
    );
}

#[test]
fn direct_projection_matches_parse_on_fixtures() {
    for source in [
        "# H1\n\n###### H6",
        "> quote\n>\n> - item",
        "1. one\n2. two",
        "3. three\n4. four",
        "- [ ] todo\n- [x] done",
        "- a\n  - b\n    1. c\n- d",
        "```rust\nfn main() {}\n```",
        "````\n```\n````",
        "---",
        "[id]: https://example.com \"Title\"",
        "a [link](https://example.com) b",
        "![alt](https://example.com/a.png)",
        "`code` and ~~strike~~ **b*bi*b** <u>u</u> <mark>m</mark>",
        "| **a** | <u>b</u> |\n| :--- | ---: |\n| *c* | d |",
        "lead  and trail ",
    ] {
        assert_equivalent(&model(source));
    }
}

#[test]
fn direct_projection_matches_parse_on_random_documents() {
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        assert_equivalent(&model(&document(&mut rng)));
    }
}
