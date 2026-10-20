//! Hover-only table grips and their menu (D20). A grip shows above the
//! hovered column's header cell and at the left edge of the hovered row;
//! clicking it selects that column / row as `BlockSelection::Cells` and
//! opens a small menu of table commands.

use gpui::{Bounds, Context, Pixels, Point, point, px, size};

use super::{
    id::BlockId,
    state::BlockEditorState,
    table::{TableEdit, TableLine},
};
use gpui_kit::base::text::editor_host::ColumnAlign;

/// Extra reach around a table where its grips still show.
const REACH: f32 = 28.;
/// Row grips sit this far left of the table frame, left of the strip that
/// node-selects the table (`block_handle_at`, the 16px left of the cell text).
const ROW_GRIP_X: f32 = 8.;

/// The open grip menu.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TableMenu {
    pub(crate) table: BlockId,
    pub(crate) line: TableLine,
    /// Window point the menu hangs from (just below / right of the grip).
    pub(crate) at: Point<Pixels>,
}

/// One menu entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MenuItem {
    Edit(TableEdit),
}

impl BlockEditorState {
    /// The grips to show for the pointer at `at`: the hovered column's grip
    /// above its header cell and the hovered row's grip at its left edge.
    /// Column and row boxes come from the frame's rebuilt table grid
    /// (`FrameLayout::layout_tables`), so they match the drawn cells for
    /// every alignment, after scrolling and resizing.
    pub fn table_grips_at(&self, at: Point<Pixels>) -> Vec<(BlockId, TableLine, Bounds<Pixels>)> {
        let frame = self.frame.borrow();
        let mut out = Vec::new();
        for g in &frame.grids {
            let b = g.bounds;
            let reach = px(REACH);
            if at.x < b.left() - reach
                || at.x >= b.right() + px(8.)
                || at.y < b.top() - reach
                || at.y >= b.bottom() + px(8.)
            {
                continue;
            }
            let x = at.x.clamp(b.left(), b.right() - px(1.));
            let y = at.y.clamp(b.top(), b.bottom() - px(1.));
            let col = g
                .cols
                .iter()
                .position(|(l, r)| x >= *l && x < *r)
                .or_else(|| (x >= b.right() - px(2.)).then(|| g.cols.len() - 1))
                .or_else(|| (x < g.cols[0].0).then_some(0));
            if let Some(c) = col {
                let (l, r) = g.cols[c];
                let mid = (l + r) / 2.;
                out.push((
                    g.table,
                    TableLine::Column(c),
                    Bounds::new(point(mid - px(8.), b.top() - px(8.)), size(px(16.), px(4.))),
                ));
            }
            let row = g
                .rows
                .iter()
                .position(|(t, bt)| y >= *t && y < *bt)
                .or_else(|| (y >= b.bottom() - px(2.)).then(|| g.rows.len() - 1))
                .or_else(|| (y < g.rows[0].0).then_some(0));
            if let Some(r) = row {
                let (t, bt) = g.rows[r];
                let h = px(16.).min(bt - t);
                let mid = (t + bt) / 2.;
                out.push((
                    g.table,
                    TableLine::Row(r),
                    Bounds::new(
                        point(b.left() - px(ROW_GRIP_X), mid - h / 2.),
                        size(px(4.), h),
                    ),
                ));
            }
            break;
        }
        out
    }

    /// The grip under `at`, if the pointer is on one (a little slack).
    pub(crate) fn table_grip_hit(
        &self,
        at: Point<Pixels>,
    ) -> Option<(BlockId, TableLine, Bounds<Pixels>)> {
        self.table_grips_at(at).into_iter().find(|(_, line, b)| {
            // Row grips get vertical slack only, so the strip right of
            // them still node-selects the table.
            let slack = match line {
                TableLine::Row(_) => Bounds::new(
                    point(b.left() - px(4.), b.top() - px(3.)),
                    size(b.size.width + px(6.), b.size.height + px(6.)),
                ),
                TableLine::Column(_) => b.dilate(px(5.)),
            };
            slack.contains(&at)
        })
    }

    /// Click on a grip: select its row / column and open the menu.
    pub(crate) fn click_table_grip(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let Some((table, line, grip)) = self.table_grip_hit(at) else {
            return false;
        };
        self.open_table_menu(table, line, grip, cx);
        true
    }

    /// Select `line` of `table` and open its menu below / beside `grip`.
    pub fn open_table_menu(
        &mut self,
        table: BlockId,
        line: TableLine,
        grip: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.model.select_table_line(table, line).unwrap_or(false) {
            let at = match line {
                TableLine::Column(_) => point(grip.left(), grip.bottom() + px(6.)),
                TableLine::Row(_) => point(grip.right() + px(6.), grip.top()),
            };
            self.table_menu = Some(TableMenu { table, line, at });
            cx.notify();
        }
    }

    /// Open the menu of `line` of `table` (the grip position from the last
    /// frame). For tests and hosts that drive the menu themselves.
    pub fn open_table_menu_for(&mut self, table: BlockId, line: TableLine, cx: &mut Context<Self>) {
        let grip = self
            .frame
            .borrow()
            .tables
            .iter()
            .find(|(t, _)| *t == table)
            .map(|(_, b)| *b)
            .unwrap_or_default();
        self.open_table_menu(table, line, grip, cx);
    }

    /// Where the open menu hangs from (window coordinates).
    pub fn table_menu_anchor(&self) -> Option<Point<Pixels>> {
        self.table_menu.as_ref().map(|m| m.at)
    }

    pub fn table_menu_open(&self) -> bool {
        self.table_menu.is_some()
    }

    pub(crate) fn close_table_menu(&mut self, cx: &mut Context<Self>) {
        if self.table_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Whether a menu entry applies; `Err` carries why it is disabled.
    pub(crate) fn table_menu_item_state(&self, item: MenuItem) -> Result<(), &'static str> {
        let Some(menu) = &self.table_menu else {
            return Err("no menu");
        };
        match (menu.line, item) {
            (TableLine::Row(0), MenuItem::Edit(TableEdit::RemoveRow)) => {
                Err("The header row can't be deleted")
            }
            _ => Ok(()),
        }
    }

    /// The current alignment of the menu's column.
    pub(crate) fn table_menu_align(&self) -> Option<ColumnAlign> {
        let menu = self.table_menu.as_ref()?;
        let TableLine::Column(c) = menu.line else {
            return None;
        };
        match self.model.doc.kind(menu.table) {
            Some(super::kind::BlockKindData::Table { aligns }) => {
                Some(aligns.get(c).copied().unwrap_or_default())
            }
            _ => None,
        }
    }

    /// Run a menu entry on the menu's row / column (one undo step) and close
    /// the menu, except for alignment, which keeps it open to show the change.
    pub fn table_menu_action(&mut self, edit: TableEdit, cx: &mut Context<Self>) {
        let Some(menu) = self.table_menu.clone() else {
            return;
        };
        if self.table_menu_item_state(MenuItem::Edit(edit)).is_err() {
            return;
        }
        // The selection may have moved since the menu opened.
        let _ = self.model.select_table_line(menu.table, menu.line);
        self.model.break_coalescing();
        let _ = self.run(super::command::EditorCommand::EditTable(edit), cx);
        self.model.break_coalescing();
        if !matches!(edit, TableEdit::SetColumnAlign(_)) {
            self.table_menu = None;
        }
        cx.notify();
    }

    /// Track which grips the pointer shows; repaint when that changes.
    pub(crate) fn hover_table(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let hovered: Vec<(BlockId, TableLine)> = self
            .table_grips_at(at)
            .into_iter()
            .map(|(t, l, _)| (t, l))
            .collect();
        let on_grip = self.table_grip_hit(at).is_some();
        if hovered != self.table_hover || on_grip != self.grip_hover {
            self.table_hover = hovered;
            self.grip_hover = on_grip;
            cx.notify();
        }
    }
}

/// Shortcut text for a menu entry (Cmd on macOS, Ctrl elsewhere).
pub(crate) fn shortcut(edit: TableEdit) -> Option<String> {
    let (m, alt) = if cfg!(target_os = "macos") {
        ("⌘", "⌥")
    } else {
        ("Ctrl+", "Alt+")
    };
    let key = match edit {
        TableEdit::AddColumnLeft => "←",
        TableEdit::AddColumnRight => "→",
        TableEdit::AddRowAbove => "↑",
        TableEdit::AddRowBelow => "↓",
        TableEdit::RemoveColumn | TableEdit::RemoveRow => {
            if cfg!(target_os = "macos") {
                "⌫"
            } else {
                "Backspace"
            }
        }
        TableEdit::SetColumnAlign(_) => return None,
    };
    Some(format!("{m}{alt}{key}"))
}

/// The open grip menu as a deferred, window-anchored popover.
pub(crate) fn render_menu(
    state: &gpui::Entity<BlockEditorState>,
    cx: &gpui::App,
) -> Option<gpui::AnyElement> {
    use gpui::prelude::FluentBuilder as _;
    use gpui::{
        InteractiveElement as _, IntoElement as _, ParentElement as _, Styled as _, anchored,
        deferred, div,
    };
    use gpui_kit::component::ActiveTheme as _;

    let s = state.read(cx);
    let menu = s.table_menu.clone()?;
    let theme = cx.theme();
    let (fg, muted, hover, border, bg, accent) = (
        theme.popover_foreground,
        theme.muted_foreground,
        theme.list_hover,
        theme.border,
        theme.popover,
        theme.accent,
    );
    let item = |id: &'static str, label: &'static str, edit: TableEdit| {
        let enabled = s.table_menu_item_state(MenuItem::Edit(edit));
        let entity = state.clone();
        div()
            .id(id)
            .flex()
            .flex_col()
            .px_2()
            .py_1()
            .rounded(px(4.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .child(div().child(label))
                    .when_some(shortcut(edit), |this, keys| {
                        this.child(div().text_color(muted).child(keys))
                    }),
            )
            .map(|this| match enabled {
                Ok(()) => this
                    .text_color(fg)
                    .hover(move |s| s.bg(hover))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        entity.update(cx, |state, cx| state.table_menu_action(edit, cx));
                    }),
                Err(why) => this.text_color(muted).child(div().text_xs().child(why)),
            })
    };
    let items: Vec<gpui::AnyElement> = match menu.line {
        TableLine::Column(_) => vec![
            item(
                "tm-col-left",
                "Insert column left",
                TableEdit::AddColumnLeft,
            )
            .into_any_element(),
            item(
                "tm-col-right",
                "Insert column right",
                TableEdit::AddColumnRight,
            )
            .into_any_element(),
            item("tm-col-delete", "Delete column", TableEdit::RemoveColumn).into_any_element(),
        ],
        TableLine::Row(_) => vec![
            item("tm-row-above", "Insert row above", TableEdit::AddRowAbove).into_any_element(),
            item("tm-row-below", "Insert row below", TableEdit::AddRowBelow).into_any_element(),
            item("tm-row-delete", "Delete row", TableEdit::RemoveRow).into_any_element(),
        ],
    };
    let current = s.table_menu_align();
    let align_row = current.map(|current| {
        let choice = |id: &'static str, label: &'static str, align: ColumnAlign| {
            let entity = state.clone();
            let on = current == align;
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded(px(4.))
                .border_1()
                .border_color(if on { accent } else { border })
                .when(on, |this| this.bg(hover))
                .text_color(fg)
                .hover(move |s| s.bg(hover))
                .child(label)
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    entity.update(cx, |state, cx| {
                        state.table_menu_action(TableEdit::SetColumnAlign(align), cx)
                    });
                })
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .pt_1()
            .mt_1()
            .border_t_1()
            .border_color(border)
            .child(div().text_color(muted).text_xs().child("Alignment"))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(choice("tm-align-left", "Left", ColumnAlign::Left))
                    .child(choice("tm-align-center", "Centre", ColumnAlign::Center))
                    .child(choice("tm-align-right", "Right", ColumnAlign::Right))
                    .child(choice("tm-align-none", "None", ColumnAlign::None)),
            )
    });
    let entity = state.clone();
    Some(
        deferred(
            anchored()
                .position(menu.at)
                .position_mode(gpui::AnchoredPositionMode::Window)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("block-editor-table-menu")
                        .occlude()
                        .min_w(px(220.))
                        .p_1()
                        .bg(bg)
                        .text_color(fg)
                        .text_sm()
                        .border_1()
                        .border_color(border)
                        .rounded(px(6.))
                        .shadow_md()
                        .on_mouse_down_out(move |_, _, cx| {
                            entity.update(cx, |state, cx| state.close_table_menu(cx));
                        })
                        .children(items)
                        .children(align_row),
                ),
        )
        .with_priority(1)
        .into_any_element(),
    )
}
