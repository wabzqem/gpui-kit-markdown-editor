//! The Markdown writer behind the serializer: delimiter-flanking aware, hoists
//! whitespace out of marked groups, escapes literal punctuation and writes
//! highlight as inline `<mark>`.
//!
//! This is the editor's own copy of the writer gpui-kit's TextView uses for
//! copy-as-source, with the options the editor needs always on. It is kept
//! here so the host API of gpui-kit stays small.

use gpui::Hsla;
use gpui_kit::base::text::editor_host::TextMark;

/// Delimiter-safe shared writer seam for links, references and images.
pub(crate) fn markdown_destination(value: &str) -> String {
    let angled = value
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '(' | ')' | '<' | '>'));
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '&' => escaped.push_str("\\&"),
            '<' => escaped.push_str("\\<"),
            '>' => escaped.push_str("\\>"),
            '\n' => escaped.push_str("&#10;"),
            '\r' => escaped.push_str("&#13;"),
            _ => escaped.push(ch),
        }
    }
    if angled {
        format!("<{escaped}>")
    } else {
        escaped
    }
}
pub(crate) fn markdown_title(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('&', "\\&")
        .replace('"', "\\\"")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;");
    format!("\"{escaped}\"")
}

/// How [`MarkdownSource`] writes its output; the serializer turns all three on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub(crate) struct MarkdownSourceOptions {
    /// Write highlight as inline `<mark>` (with a `color` attribute unless it
    /// is the default colour) instead of `==x==`, which the parser reads back
    /// as literal text.
    pub(crate) highlight_as_html: bool,
    /// Move leading and trailing whitespace of a marked group outside its
    /// delimiters: `**a **b` is literal text, `**a** b` is emphasis.
    pub(crate) hoist_whitespace: bool,
    /// Backslash-escape Markdown punctuation in text pushed with
    /// [`MarkdownSource::push_literal`], so literal `*` stays literal.
    pub(crate) escape_literals: bool,
}

/// The highlight colour `<mark>` gets when it carries no colour of its own
/// (`format::html::default_mark_color`).
fn default_highlight_color() -> Hsla {
    gpui::rgb(0xfef08a).into()
}

/// `#rrggbb`, or `#rrggbbaa` when the colour is not opaque.
pub(crate) fn hex_color(color: Hsla) -> String {
    let rgba = gpui::Rgba::from(color);
    let byte = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    let (r, g, b, a) = (byte(rgba.r), byte(rgba.g), byte(rgba.b), byte(rgba.a));
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// Keep syntax and marks separate until all selected runs and atomic objects
/// have been collected. Wrapping every run independently creates adjacent `*`
/// delimiters that can turn italic formulas into bold ones when pasted.
#[derive(Default)]
pub(crate) struct MarkdownSource {
    pieces: Vec<(String, Vec<TextMark>)>,
    options: MarkdownSourceOptions,
}

impl MarkdownSource {
    pub(crate) fn with_options(options: MarkdownSourceOptions) -> Self {
        Self {
            pieces: Vec::new(),
            options,
        }
    }

    /// Push plain text (not Markdown source) carrying `mark`. With
    /// `escape_literals`, Markdown punctuation is backslash-escaped, except
    /// inside code, where escapes are not interpreted.
    pub(crate) fn push_literal(&mut self, text: &str, mark: &TextMark) {
        if !self.options.escape_literals || mark.code {
            self.push_marked(text, mark);
            return;
        }
        let mut escaped = String::with_capacity(text.len());
        for ch in text.chars() {
            if ch == '\n' {
                // Hard breaks, including terminal/table/styled breaks, need
                // an explicit semantic form. Backslash-newline at EOF reparses
                // as a literal backslash; whitespace hoisting can break marks.
                escaped.push_str("<br>");
                continue;
            }
            if matches!(
                ch,
                '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '~' | '$' | '&' | '|'
            ) {
                escaped.push('\\');
            }
            escaped.push(ch);
        }
        self.push_marked(&escaped, mark);
    }

    pub(crate) fn push_marked(&mut self, source: &str, mark: &TextMark) {
        if source.is_empty() {
            return;
        }
        // Outer-to-inner order for equally extensive marks. When a mark spans
        // more neighboring pieces, serialize it outside the shorter marks.
        let mut layers = Vec::new();
        if let Some(link) = &mark.link {
            layers.push(TextMark {
                link: Some(link.clone()),
                ..Default::default()
            });
        }
        if let Some(highlight) = mark.highlight {
            layers.push(TextMark {
                highlight: Some(highlight),
                ..Default::default()
            });
        }
        if mark.underline {
            layers.push(TextMark::default().underline());
        }
        if mark.strikethrough {
            layers.push(TextMark::default().strikethrough());
        }
        if mark.bold {
            layers.push(TextMark::default().bold());
        }
        if mark.italic {
            layers.push(TextMark::default().italic());
        }
        if mark.code {
            layers.push(TextMark::default().code());
        }
        self.pieces.push((source.to_string(), layers));
    }

    pub(crate) fn finish(mut self) -> String {
        /// `before` / `after`: the characters just outside `pieces`, for the
        /// delimiter flanking check (`None` = start or end of the text).
        fn write(
            pieces: &mut [(String, Vec<TextMark>)],
            options: MarkdownSourceOptions,
            before: Option<char>,
            after: Option<char>,
        ) -> String {
            let mut out = String::new();
            let mut offset = 0;
            let mut prev_marked = false;
            while offset < pieces.len() {
                // Nothing inside a code span is parsed, so in the editor's
                // mode code is always the innermost layer.
                let only_code = |mark: &&TextMark| {
                    !options.hoist_whitespace || !mark.code || pieces[offset].1.len() == 1
                };
                let Some((mark, count)) = pieces[offset]
                    .1
                    .iter()
                    .filter(only_code)
                    .map(|mark| {
                        let count = pieces[offset..]
                            .iter()
                            .take_while(|(_, layers)| {
                                layers.contains(mark)
                                    && (!options.hoist_whitespace
                                        || !mark.code
                                        || layers.len() == 1)
                            })
                            .count();
                        (mark.clone(), count)
                    })
                    .reduce(|best, candidate| {
                        if candidate.1 > best.1 {
                            candidate
                        } else {
                            best
                        }
                    })
                else {
                    out.push_str(&pieces[offset].0);
                    offset += 1;
                    prev_marked = false;
                    continue;
                };
                // A directly adjacent marked neighbour has delimiters of its
                // own that could merge with ours into one run (`***`), so
                // count it as a word character: only a non-punctuation edge
                // is then safe.
                let outside_before = if prev_marked {
                    Some('a')
                } else {
                    out.chars().last().or(before)
                };
                let outside_after = match pieces.get(offset + count) {
                    Some((_, layers)) if !layers.is_empty() => Some('a'),
                    Some((text, _)) => text.chars().next(),
                    None => after,
                };
                let group = &mut pieces[offset..offset + count];
                for (_, layers) in group.iter_mut() {
                    layers.retain(|layer| layer != &mark);
                }
                let content = write(group, options, Some('*'), Some('*'));
                out.push_str(&wrap_layer(
                    &content,
                    &mark,
                    options,
                    outside_before,
                    outside_after,
                ));
                offset += count;
                prev_marked = true;
            }
            out
        }
        let options = self.options;
        write(&mut self.pieces, options, None, None)
    }
}

/// Wrap one mark layer around already-written content, per `options`.
///
/// With `hoist_whitespace`, delimiters are also kept *flanking* in the
/// CommonMark sense: when `*`, `**` or `~~` would sit between a word
/// character outside and punctuation inside (`a*`code`*`), it could not open
/// or close, so the HTML form the parser reads back (`<em>`, `<strong>`,
/// `<s>`) is used instead.
fn wrap_layer(
    content: &str,
    mark: &TextMark,
    options: MarkdownSourceOptions,
    before: Option<char>,
    after: Option<char>,
) -> String {
    if mark.code && options.escape_literals {
        // Code delimiters must not collide with literal backtick runs. Code
        // whitespace belongs inside the span, never hoisted into prose.
        let longest = content
            .split(|ch| ch != '`')
            .map(str::len)
            .max()
            .unwrap_or(0);
        let fence = "`".repeat(longest + 1);
        let pad = content.starts_with('`')
            || content.ends_with('`')
            || (content.starts_with(' ')
                && content.ends_with(' ')
                && content.chars().any(|ch| ch != ' '));
        return if pad {
            format!("{fence} {content} {fence}")
        } else {
            format!("{fence}{content}{fence}")
        };
    }
    let (lead, core, trail) = if options.hoist_whitespace {
        let core = content.trim_matches(char::is_whitespace);
        if core.is_empty() {
            return content.to_string();
        }
        let start = content.len() - content.trim_start_matches(char::is_whitespace).len();
        (&content[..start], core, &content[start + core.len()..])
    } else {
        ("", content, "")
    };
    let wrapped = match mark.highlight {
        Some(color) if options.highlight_as_html && !core.is_empty() => {
            if color == default_highlight_color() {
                format!("<mark>{core}</mark>")
            } else {
                format!("<mark color=\"{}\">{core}</mark>", hex_color(color))
            }
        }
        _ if options.hoist_whitespace
            && (mark.bold || mark.italic || mark.strikethrough)
            && !delimiters_flank(
                lead.chars().last().or(before),
                core,
                trail.chars().next().or(after),
            ) =>
        {
            let tag = if mark.bold {
                "strong"
            } else if mark.italic {
                "em"
            } else {
                "s"
            };
            format!("<{tag}>{core}</{tag}>")
        }
        _ => wrap_with_mark(core, mark),
    };
    format!("{lead}{wrapped}{trail}")
}

/// Whether delimiters around `core` would be left-flanking at the start and
/// right-flanking at the end, given the characters outside.
fn delimiters_flank(before: Option<char>, core: &str, after: Option<char>) -> bool {
    let weak =
        |ch: Option<char>| ch.is_none_or(|ch| ch.is_whitespace() || ch.is_ascii_punctuation());
    let (Some(first), Some(last)) = (core.chars().next(), core.chars().last()) else {
        return true;
    };
    let opens = !first.is_whitespace() && (!first.is_ascii_punctuation() || weak(before));
    let closes = !last.is_whitespace() && (!last.is_ascii_punctuation() || weak(after));
    opens && closes
}

/// Wrap `text` with the Markdown syntax implied by `mark`, innermost first.
fn wrap_with_mark(text: &str, mark: &TextMark) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut out = text.to_string();
    if mark.code {
        out = format!("`{}`", out);
    }
    if mark.italic {
        out = format!("*{}*", out);
    }
    if mark.bold {
        out = format!("**{}**", out);
    }
    if mark.strikethrough {
        out = format!("~~{}~~", out);
    }
    if mark.underline {
        // Markdown has no underline syntax, and `__` reads as bold to most
        // parsers, so fall back to the inline HTML `<u>` parses from.
        out = format!("<u>{}</u>", out);
    }
    if mark.highlight.is_some() {
        out = format!("=={}==", out);
    }
    if let Some(link) = &mark.link {
        out = match &link.title {
            Some(title) => format!(
                "[{}]({} {})",
                out,
                markdown_destination(&link.url),
                markdown_title(title)
            ),
            None => format!("[{}]({})", out, markdown_destination(&link.url)),
        };
    }
    out
}
