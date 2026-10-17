//! Small helpers the editor keeps its own copy of (word boundaries for
//! double-click, the caret blink timer, caret width and pixel snapping), so
//! gpui-kit does not have to expose its input internals.

use std::{ops::Range, time::Duration};

use gpui::{Bounds, Context, Pixels, Task, point, px, size};

// ---- Word ranges (same rules as the kit's text selection) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CharType {
    /// a-z, A-Z, 0-9, _
    Word,
    /// '\t', ' ', '\u{00A0}' etc.
    Whitespace,
    /// \n, \r
    Newline,
    /// . , ; : ( ) [ ] { } ... or CJK characters: `汉`, `🎉` etc.
    Other,
}

/// Implementation based on <https://github.com/zed-industries/zed/blob/main/crates/gpui/src/text_system/line_wrapper.rs>
fn is_word_char(c: char) -> bool {
    matches!(c, '_')
        // ASCII alphanumeric characters, for English, numbers: `Hello123`, etc.
        || c.is_ascii_alphanumeric()
        // Latin script in Unicode for French, German, Spanish, etc.
        || matches!(c, '\u{00C0}'..='\u{00FF}')
        || matches!(c, '\u{0100}'..='\u{017F}')
        || matches!(c, '\u{0180}'..='\u{024F}')
        // Cyrillic for Russian, Ukrainian, etc.
        || matches!(c, '\u{0400}'..='\u{04FF}')
        // Vietnamese
        || matches!(c, '\u{1E00}'..='\u{1EFF}')
        || matches!(c, '\u{0300}'..='\u{036F}')
}

impl From<char> for CharType {
    fn from(c: char) -> Self {
        match c {
            c if is_word_char(c) => CharType::Word,
            c if c == '\n' || c == '\r' => CharType::Newline,
            c if c.is_whitespace() => CharType::Whitespace,
            _ => CharType::Other,
        }
    }
}

impl CharType {
    fn is_connectable(self, c: char) -> bool {
        matches!(
            (self, CharType::from(c)),
            (CharType::Word, CharType::Word) | (CharType::Whitespace, CharType::Whitespace)
        )
    }
}

pub(crate) fn word_range_from_chars(
    offset: usize,
    c: char,
    prev_chars: impl Iterator<Item = char>,
    next_chars: impl Iterator<Item = char>,
) -> Range<usize> {
    let char_type = CharType::from(c);
    let mut start = offset;
    let mut end = offset + c.len_utf8();

    for prev in prev_chars.take(128) {
        if char_type.is_connectable(prev) {
            start -= prev.len_utf8();
        } else {
            break;
        }
    }

    for next in next_chars.take(128) {
        if char_type.is_connectable(next) {
            end += next.len_utf8();
        } else {
            break;
        }
    }

    start..end
}

pub(crate) fn word_range_at(text: &str, offset: usize) -> Option<Range<usize>> {
    if text.is_empty() {
        return None;
    }

    let offset = clip_offset(text, offset);
    let c = text[offset..].chars().next()?;
    Some(word_range_from_chars(
        offset,
        c,
        text[..offset].chars().rev(),
        text[offset + c.len_utf8()..].chars(),
    ))
}

fn clip_offset(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return text.char_indices().next_back().map_or(0, |(ix, _)| ix);
    }

    if text.is_char_boundary(offset) {
        offset
    } else {
        text.char_indices()
            .map(|(ix, _)| ix)
            .take_while(|ix| *ix < offset)
            .last()
            .unwrap_or_default()
    }
}

// ---- Caret ----

static INTERVAL: Duration = Duration::from_millis(500);
static PAUSE_DELAY: Duration = Duration::from_millis(300);

pub(crate) const CURSOR_WIDTH: Pixels = px(1.);

/// To manage the Input cursor blinking.
///
/// It will start blinking with a interval of 500ms.
/// Every loop will notify the view to update the `visible`, and Input will observe this update to touch repaint.
///
/// The input painter will check if this in visible state, then it will draw the cursor.
pub(crate) struct BlinkCursor {
    visible: bool,
    paused: bool,
    epoch: usize,

    _task: Task<()>,
}

impl BlinkCursor {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            paused: false,
            epoch: 0,
            _task: Task::ready(()),
        }
    }

    /// Start the blinking from a visible cursor.
    ///
    /// A single focus can start the cursor more than once: `InputState::focus`
    /// starts it and then the focus observer starts it again, and window
    /// activation starts it too. Resuming the running cycle would toggle the
    /// cursor that was just shown off again, so every start restarts the cycle.
    pub(crate) fn start(&mut self, cx: &mut Context<Self>) {
        self.paused = false;
        self.visible = false;
        let epoch = self.next_epoch();
        self.blink(epoch, cx);
    }

    /// Stop the blinking and clear the blink state, so the next [`Self::start`]
    /// begins from a visible cursor instead of resuming a stale pause.
    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        self.epoch = 0;
        self.paused = false;
        self.visible = false;
        self._task = Task::ready(());
        cx.notify();
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch += 1;
        self.epoch
    }

    fn blink(&mut self, epoch: usize, cx: &mut Context<Self>) {
        // A task from an earlier blink lifecycle must not mutate the current state.
        if epoch != self.epoch {
            return;
        }

        if self.paused {
            self.visible = true;
            return;
        }

        self.visible = !self.visible;
        cx.notify();

        // Schedule the next blink
        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(INTERVAL).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.blink(epoch, cx));
            }
        });
    }

    pub(crate) fn visible(&self) -> bool {
        // Keep showing the cursor if paused
        self.paused || self.visible
    }

    /// Show the cursor immediately and restart the idle delay before blinking resumes.
    ///
    /// This is a no-op on a cursor that is not blinking. `epoch` is zero only
    /// before the first [`Self::start`] and after [`Self::stop`], so a zero
    /// epoch means the input is not focused, and there is no cursor on screen
    /// to keep visible. Pausing it anyway would start a blink loop that no blur
    /// is left to stop, and every blink repaints the view that the input is in.
    pub(crate) fn pause(&mut self, cx: &mut Context<Self>) {
        if self.epoch == 0 {
            return;
        }

        self.paused = true;
        self.visible = true;
        cx.notify();

        // Every pause replaces the pending timer, keeping repeated input visible.
        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PAUSE_DELAY).await;

            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    if epoch != this.epoch {
                        return;
                    }

                    this.paused = false;
                    this.blink(epoch, cx);
                });
            }
        });
    }
}

/// Snap the caret to whole device pixels, at least one pixel wide.
pub(crate) fn snap_caret_to_device_pixels(
    bounds: Bounds<Pixels>,
    scale_factor: f32,
) -> Bounds<Pixels> {
    let x = (bounds.origin.x * scale_factor).round() / scale_factor;
    let width = (bounds.size.width * scale_factor).round().max(px(1.)) / scale_factor;
    Bounds::new(point(x, bounds.origin.y), size(width, bounds.size.height))
}
