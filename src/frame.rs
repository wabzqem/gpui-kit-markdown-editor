//! Per-frame geometry, collected through gpui-kit's editor host API.
//!
//! During the editor's prepaint every keyed text leaf reports a
//! [`LeafGeometry`] to `collect_leaf_geometry`, which also paints the
//! selection ranges handed to it. After prepaint the geometries become the
//! editor's [`FrameLayout`], which handlers hit-test against until the next
//! frame. Everything here maps between the model's offsets and the leaf
//! indices gpui-kit reports.

use std::{collections::HashMap, ops::Range, rc::Rc};

use gpui::{Bounds, Pixels, Point, point, px, size};
use gpui_kit::base::text::editor_host::LeafGeometry;

use super::{id::BlockId, position::BlockPosition, projection::LeafInfo};

/// Builds the frame from what `collect_leaf_geometry` returned.
pub(crate) fn into_frame(
    mut geometries: HashMap<u64, LeafGeometry>,
    leaves: Rc<Vec<LeafInfo>>,
    tables: &[(BlockId, Vec<BlockId>)],
) -> FrameLayout {
    let mut frame = FrameLayout::default();
    for (table, cells) in tables {
        let bounds = cells
            .iter()
            .filter_map(|cell| geometries.get(&cell.as_u64()).map(|g| g.bounds()))
            .reduce(|a, b| a.union(&b));
        if let Some(bounds) = bounds {
            frame.tables.push((*table, bounds));
        }
    }
    for info in leaves.iter() {
        if let Some(geometry) = geometries.remove(&info.block.as_u64()) {
            frame.index.insert(info.block, frame.leaves.len());
            frame.leaves.push(LeafGeom {
                info: *info,
                geometry,
            });
        }
    }
    frame
}

pub(crate) struct LeafGeom {
    pub(crate) info: LeafInfo,
    geometry: LeafGeometry,
}

impl LeafGeom {
    pub(crate) fn bounds(&self) -> Bounds<Pixels> {
        self.geometry.bounds()
    }

    pub(crate) fn line_height(&self) -> Pixels {
        self.geometry.line_height()
    }

    /// Height of the caret: the font's ascent + descent (like a text
    /// input's caret), not the whole line box, which includes leading.
    pub(crate) fn caret_height(&self) -> Pixels {
        self.geometry.caret_height()
    }

    /// Rendered index for a model offset.
    fn index(&self, offset: usize) -> usize {
        let len = self.geometry.len();
        if self.info.atomic {
            if offset == 0 { 0 } else { len }
        } else {
            offset.min(self.info.len).min(len)
        }
    }

    /// Top of the caret at `offset`, window coordinates.
    pub(crate) fn caret_point(&self, offset: usize) -> Point<Pixels> {
        self.geometry.position_for_index(self.index(offset))
    }

    /// Closest model offset for a window point.
    pub(crate) fn offset_at(&self, at: Point<Pixels>) -> usize {
        let index = self.geometry.index_for_position(at);
        if self.info.atomic {
            usize::from(index * 2 > self.geometry.len().max(1))
        } else {
            index.min(self.info.len)
        }
    }

    /// The inline atom drawn under `at`, as its leaf byte range.
    pub(crate) fn atom_at(&self, at: Point<Pixels>) -> Option<Range<usize>> {
        self.geometry.atom_at(at)
    }
}

/// The last complete frame's leaf geometry, in document order.
#[derive(Default)]
pub(crate) struct FrameLayout {
    pub(crate) leaves: Vec<LeafGeom>,
    pub(crate) index: HashMap<BlockId, usize>,
    /// Table bounds (union of its cells), for node selection by click.
    pub(crate) tables: Vec<(BlockId, Bounds<Pixels>)>,
    /// The editor's content area: tables span its width, while their cells'
    /// text bounds only cover the text.
    pub(crate) content: Option<Bounds<Pixels>>,
}

impl FrameLayout {
    pub(crate) fn leaf(&self, block: BlockId) -> Option<&LeafGeom> {
        self.index.get(&block).map(|ix| &self.leaves[*ix])
    }

    pub(crate) fn caret_bounds(&self, pos: BlockPosition, width: Pixels) -> Option<Bounds<Pixels>> {
        let leaf = self.leaf(pos.block)?;
        let (line, h) = (leaf.line_height(), leaf.caret_height());
        let mut origin = leaf.caret_point(pos.offset);
        // GPUI centres the glyphs in the line box: so does the caret.
        origin.y += (line - h) / 2.;
        Some(Bounds::new(origin, size(width, h)))
    }

    /// The table under a window point.
    /// Kept for the block handle (not built yet); clicks in a table place the caret.
    #[allow(dead_code)]
    pub(crate) fn table_at(&self, at: Point<Pixels>) -> Option<BlockId> {
        self.tables
            .iter()
            .find(|(_, bounds)| bounds.contains(&at))
            .map(|(id, _)| *id)
    }

    /// Nearest caret position to a window point.
    pub(crate) fn hit_test(&self, at: Point<Pixels>) -> Option<BlockPosition> {
        let last = self.leaves.len().checked_sub(1)?;
        let ix = self
            .leaves
            .partition_point(|leaf| leaf.bounds().bottom() <= at.y)
            .min(last);
        let ix = self.nearest_in_band(ix, at.x);
        let leaf = &self.leaves[ix];
        Some(BlockPosition::new(leaf.info.block, leaf.offset_at(at)))
    }

    /// Leaves side by side with `ix` (table cells in one row share a
    /// vertical band): the one horizontally nearest to `x`.
    fn nearest_in_band(&self, ix: usize, x: Pixels) -> usize {
        let band = self.leaves[ix].bounds();
        let overlaps = |i: usize| {
            let b = self.leaves[i].bounds();
            b.top() < band.bottom() && b.bottom() > band.top()
        };
        let mut lo = ix;
        while lo > 0 && overlaps(lo - 1) {
            lo -= 1;
        }
        let mut hi = ix;
        while hi + 1 < self.leaves.len() && overlaps(hi + 1) {
            hi += 1;
        }
        let distance = |i: usize| {
            let b = self.leaves[i].bounds();
            if x < b.left() {
                b.left() - x
            } else if x > b.right() {
                x - b.right()
            } else {
                px(0.)
            }
        };
        (lo..=hi)
            .min_by(|a, b| {
                distance(*a)
                    .partial_cmp(&distance(*b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(ix)
    }

    /// One visual line above/below `pos`, keeping `x`.
    pub(crate) fn vertical(
        &self,
        pos: BlockPosition,
        x: Pixels,
        down: bool,
    ) -> Option<BlockPosition> {
        let ix = *self.index.get(&pos.block)?;
        let leaf = &self.leaves[ix];
        let caret = leaf.caret_point(pos.offset);
        let lh = leaf.line_height();
        let y = if down {
            caret.y + lh * 1.5
        } else {
            caret.y - lh * 0.5
        };
        let bounds = leaf.bounds();
        if y >= bounds.top() && y < bounds.bottom() {
            return Some(BlockPosition::new(
                leaf.info.block,
                leaf.offset_at(point(x, y)),
            ));
        }
        // The next leaf above/below that is not side by side with this one
        // (skips the rest of a table row), then the nearest in its band.
        let side_by_side = |i: usize| {
            let b = self.leaves[i].bounds();
            b.top() < bounds.bottom() && b.bottom() > bounds.top()
        };
        let mut j = ix;
        loop {
            j = if down {
                Some(j + 1).filter(|j| *j < self.leaves.len())?
            } else {
                j.checked_sub(1)?
            };
            if !side_by_side(j) {
                break;
            }
        }
        let next = &self.leaves[self.nearest_in_band(j, x)];
        let b = next.bounds();
        let y = if down {
            b.top() + next.line_height() / 2.
        } else {
            b.bottom() - next.line_height() / 2.
        };
        Some(BlockPosition::new(
            next.info.block,
            next.offset_at(point(x, y)),
        ))
    }

    /// Start or end of the visual (soft-wrapped) line holding `pos`.
    pub(crate) fn line_edge(&self, pos: BlockPosition, end: bool) -> Option<BlockPosition> {
        let leaf = self.leaf(pos.block)?;
        let caret = leaf.caret_point(pos.offset);
        let b = leaf.bounds();
        let x = if end {
            b.right() + px(10000.)
        } else {
            b.left() - px(1.)
        };
        let y = caret.y + leaf.line_height() / 2.;
        Some(BlockPosition::new(pos.block, leaf.offset_at(point(x, y))))
    }
}
