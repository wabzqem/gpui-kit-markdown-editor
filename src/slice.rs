//! Copy of a selection that spans several blocks, as Markdown.
//!
//! The selected part of the tree is cut into detached blocks: containers keep
//! their kind (list item, task state, quote), partial first and last leaves
//! are cut with balanced marks, and anything outside the selection is
//! dropped. The result is serialized by the ordinary writer, so a paste that
//! parses it back gets the same structure.
//!
//! Cut rules:
//! - A boundary leaf cut to nothing is dropped (selecting from the end of a
//!   paragraph does not copy an empty one).
//! - A table the selection touches is copied whole: a table with missing
//!   cells has no Markdown form.
//! - A list item whose own text lies before the selection (the selection
//!   starts inside its nested list) is not copied as an empty item; its
//!   nested content is lifted out instead.

use crate::{
    document::{BlockContent, EditorDocument},
    id::BlockId,
    kind::BlockKindData,
    selection::{BlockSelection, ResolvedSelection},
    step::{DetachedBlock, DetachedContent},
    transaction::TransactionBuilder,
};

/// The Markdown for the blocks and partial blocks in `range`, or `None` when
/// nothing is selected.
pub(crate) fn selection_markdown(doc: &EditorDocument, range: ResolvedSelection) -> Option<String> {
    let (Some(first), Some(last)) = (
        doc.order.rank(range.start.block),
        doc.order.rank(range.end.block),
    ) else {
        return None;
    };
    let cut = Cut {
        doc,
        range: &range,
        first,
        last,
    };
    let blocks: Vec<DetachedBlock> = doc
        .children(doc.root)
        .iter()
        .flat_map(|id| cut.block(*id))
        .collect();
    if blocks.is_empty() {
        return None;
    }
    let mut scratch = EditorDocument::empty(doc.extensions.clone());
    let count = blocks.len();
    let root = scratch.root;
    let mut tx = TransactionBuilder::new(&mut scratch);
    tx.insert(root, 0, blocks).ok()?;
    tx.remove(root, count..count + 1).ok()?;
    let caret = BlockSelection::caret(tx.doc.first_caret());
    tx.finish(
        crate::transaction::EditOrigin::Command("copy"),
        caret.clone(),
        caret,
    )
    .ok()?;
    let ids = scratch.children(root).to_vec();
    Some(crate::serialize::join_blocks(&scratch, &ids))
}

struct Cut<'a> {
    doc: &'a EditorDocument,
    range: &'a ResolvedSelection,
    first: u32,
    last: u32,
}

impl Cut<'_> {
    fn touches(&self, id: BlockId) -> bool {
        self.doc
            .order
            .rank(id)
            .is_some_and(|r| r >= self.first && r <= self.last)
    }

    fn block(&self, id: BlockId) -> Vec<DetachedBlock> {
        let Some(entry) = self.doc.get(id) else {
            return Vec::new();
        };
        let children = self.doc.children(id);
        if matches!(entry.content, BlockContent::Children(_)) {
            return self.container(id, &entry.kind, children);
        }
        if !self.touches(id) {
            return Vec::new();
        }
        let start = (id == self.range.start.block).then_some(self.range.start.offset);
        let end = (id == self.range.end.block).then_some(self.range.end.offset);
        let boundary = start.is_some() || end.is_some();
        let content = match &entry.content {
            BlockContent::Inline(inline) => {
                let lo = start.unwrap_or(0);
                let hi = end.unwrap_or(inline.len());
                let cut = inline.slice(lo..hi);
                if boundary && cut.is_empty() {
                    return Vec::new();
                }
                BlockContent::Inline(cut)
            }
            BlockContent::Plain(text) => {
                let lo = start.unwrap_or(0);
                let hi = end.unwrap_or(text.len());
                let cut = text.get(lo..hi).unwrap_or_default();
                if boundary && cut.is_empty() {
                    return Vec::new();
                }
                BlockContent::Plain(cut.to_string())
            }
            other => other.clone(),
        };
        vec![DetachedBlock::leaf(entry.kind.clone(), content)]
    }

    fn container(
        &self,
        id: BlockId,
        kind: &BlockKindData,
        children: &[BlockId],
    ) -> Vec<DetachedBlock> {
        if matches!(kind, BlockKindData::Table { .. }) {
            let doc = self.doc;
            let touched = match (
                doc.order.rank(doc.first_leaf_in(id).block),
                doc.order.rank(doc.last_leaf_in(id).block),
            ) {
                (Some(a), Some(b)) => a <= self.last && b >= self.first,
                _ => false,
            };
            return if touched {
                vec![self.copy_whole(id)]
            } else {
                Vec::new()
            };
        }
        let mut cut: Vec<DetachedBlock> = Vec::new();
        for child in children {
            let pieces = self.block(*child);
            match (
                kind,
                matches!(self.doc.kind(*child), Some(BlockKindData::ListItem { .. })),
            ) {
                // A list item that kept only nested blocks (its own text was
                // before the selection) is lifted out of the list.
                (BlockKindData::List { .. }, true) if headless(&pieces) => {
                    for piece in pieces {
                        if let DetachedContent::Children(inner) = piece.content {
                            cut.extend(inner);
                        }
                    }
                }
                _ => cut.extend(pieces),
            }
        }
        if cut.is_empty() {
            return Vec::new();
        }
        // The lifted blocks of a headless first item come before the rest of
        // the list; keep the remaining items together in one list.
        if matches!(kind, BlockKindData::List { .. }) {
            let (lifted, items): (Vec<_>, Vec<_>) = cut
                .into_iter()
                .partition(|b| !matches!(b.kind, BlockKindData::ListItem { .. }));
            let mut out = lifted;
            if !items.is_empty() {
                out.push(DetachedBlock::container(kind.clone(), items));
            }
            return out;
        }
        vec![DetachedBlock::container(kind.clone(), cut)]
    }

    fn copy_whole(&self, id: BlockId) -> DetachedBlock {
        let Some(entry) = self.doc.get(id) else {
            return DetachedBlock::paragraph(Default::default());
        };
        match &entry.content {
            BlockContent::Children(children) => DetachedBlock::container(
                entry.kind.clone(),
                children.iter().map(|c| self.copy_whole(*c)).collect(),
            ),
            other => DetachedBlock::leaf(entry.kind.clone(), other.clone()),
        }
    }
}

/// A cut list item with no text of its own: its first child is not a leaf.
fn headless(pieces: &[DetachedBlock]) -> bool {
    match pieces.first() {
        Some(item) => match &item.content {
            DetachedContent::Children(children) => !matches!(
                children.first().map(|c| &c.content),
                Some(DetachedContent::Leaf(_))
            ),
            DetachedContent::Leaf(_) => false,
        },
        None => false,
    }
}
