// Markdown for the transcript: pulldown-cmark parses a reply into blocks whose text carries
// styled spans, and `render` turns those into GPUI text. Same shape as zeron's markdown stack
// (MIT, see THIRD_PARTY_NOTICES.md): a block tree with flattened inline runs, drawn as styled
// text. A streaming reply is simply parsed again as it grows; replies are short enough.

mod code;
mod fade;
pub mod find;
pub mod links;
mod mend;
mod render;
pub mod select;

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, TextMergeStream};

pub use fade::Fade;
pub use render::{Unit, part, render, units};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code {
        lang: String,
        text: String,
    },
    List {
        /// The first number of an ordered list.
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Quote(Vec<Block>),
    Rule,
    Table {
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
    /// A picture standing on its own line: where it is, and its alt text.
    Image {
        url: String,
        alt: String,
    },
}

/// A run of text and the styled spans over it. Spans never overlap and are in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
}

impl Style {
    fn plain(&self) -> bool {
        *self == Style::default()
    }
}

impl Inline {
    fn push(&mut self, text: &str, style: &Style) {
        let start = self.text.len();
        self.text.push_str(text);
        if style.plain() || text.is_empty() {
            return;
        }
        match self.spans.last_mut() {
            Some((r, s)) if r.end == start && s == style => r.end = self.text.len(),
            _ => self.spans.push((start..self.text.len(), style.clone())),
        }
    }
}

/// What the parser is filling: block containers on a stack, and the inline text of the block
/// being read.
enum Frame {
    Blocks(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
    Table {
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
        in_head: bool,
    },
}

enum Open {
    Paragraph,
    Heading(u8),
    Cell,
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    open: Option<(Open, Inline)>,
    code: Option<(String, String)>,
    style: Style,
    bold: u32,
    italic: u32,
    strike: u32,
    links: Vec<String>,
    /// An image being read: where it is, and its alt text so far.
    image: Option<(String, String)>,
}

impl Builder {
    fn blocks(&mut self) -> &mut Vec<Block> {
        let holds_blocks = matches!(
            self.stack.last(),
            Some(Frame::Blocks(_) | Frame::Quote(_) | Frame::Item(_))
        );
        // text straight inside a list or table frame: give it a place to go
        if !holds_blocks {
            self.stack.push(Frame::Item(Vec::new()));
        }
        match self.stack.last_mut() {
            Some(Frame::Blocks(b) | Frame::Quote(b) | Frame::Item(b)) => b,
            _ => unreachable!("a block frame was just pushed"),
        }
    }

    fn restyle(&mut self) {
        self.style = Style {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strike: self.strike > 0,
            code: false,
            link: self.links.last().cloned(),
        };
    }

    fn text(&mut self, text: &str, code: bool) {
        if let Some((_, buf)) = self.code.as_mut() {
            buf.push_str(text);
            return;
        }
        if let Some((_, alt)) = self.image.as_mut() {
            alt.push_str(text);
            return;
        }
        if self.open.is_none() {
            // a tight list item's text comes with no paragraph around it
            self.open = Some((Open::Paragraph, Inline::default()));
        }
        let mut style = self.style.clone();
        style.code = code;
        let Some((_, inline)) = self.open.as_mut() else {
            return;
        };
        if code || style.link.is_some() {
            inline.push(text, &style);
            return;
        }
        for (part, url) in bare_urls(text) {
            style.link = url;
            inline.push(part, &style);
        }
    }

    fn close_inline(&mut self) {
        let Some((kind, inline)) = self.open.take() else {
            return;
        };
        match kind {
            Open::Paragraph => self.blocks().push(Block::Paragraph(inline)),
            Open::Heading(level) => self.blocks().push(Block::Heading(level, inline)),
            Open::Cell => match self.stack.last_mut() {
                Some(Frame::Table {
                    head,
                    rows,
                    in_head,
                }) => {
                    if *in_head {
                        head.push(inline);
                    } else if let Some(row) = rows.last_mut() {
                        row.push(inline);
                    }
                }
                _ => self.blocks().push(Block::Paragraph(inline)),
            },
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                self.close_inline();
                self.open = Some((Open::Paragraph, Inline::default()));
            }
            Tag::Heading { level, .. } => {
                self.close_inline();
                self.open = Some((Open::Heading(level as u8), Inline::default()));
            }
            Tag::CodeBlock(kind) => {
                self.close_inline();
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").into(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::BlockQuote(_) => {
                self.close_inline();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::List(start) => {
                self.close_inline();
                self.stack.push(Frame::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.close_inline();
                self.stack.push(Frame::Item(Vec::new()));
            }
            Tag::Table(_) => {
                self.close_inline();
                self.stack.push(Frame::Table {
                    head: Vec::new(),
                    rows: Vec::new(),
                    in_head: false,
                });
            }
            Tag::TableHead => {
                if let Some(Frame::Table { in_head, .. }) = self.stack.last_mut() {
                    *in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(Frame::Table { rows, .. }) = self.stack.last_mut() {
                    rows.push(Vec::new());
                }
            }
            Tag::TableCell => self.open = Some((Open::Cell, Inline::default())),
            Tag::Strong => {
                self.bold += 1;
                self.restyle();
            }
            Tag::Emphasis => {
                self.italic += 1;
                self.restyle();
            }
            Tag::Strikethrough => {
                self.strike += 1;
                self.restyle();
            }
            Tag::Link { dest_url, .. } => {
                self.links.push(dest_url.to_string());
                self.restyle();
            }
            Tag::Image { dest_url, .. } => self.image = Some((dest_url.to_string(), String::new())),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell => self.close_inline(),
            TagEnd::CodeBlock => {
                if let Some((lang, mut text)) = self.code.take() {
                    if text.ends_with('\n') {
                        text.pop();
                    }
                    self.blocks().push(Block::Code { lang, text });
                }
            }
            TagEnd::BlockQuote(_) => {
                self.close_inline();
                if let Some(Frame::Quote(b)) = self.stack.pop() {
                    self.blocks().push(Block::Quote(b));
                }
            }
            TagEnd::Item => {
                self.close_inline();
                if let Some(Frame::Item(b)) = self.stack.pop()
                    && let Some(Frame::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(b);
                }
            }
            TagEnd::List(_) => {
                self.close_inline();
                if let Some(Frame::List { start, items }) = self.stack.pop() {
                    self.blocks().push(Block::List { start, items });
                }
            }
            TagEnd::TableHead => {
                if let Some(Frame::Table { in_head, .. }) = self.stack.last_mut() {
                    *in_head = false;
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table { head, rows, .. }) = self.stack.pop() {
                    self.blocks().push(Block::Table { head, rows });
                }
            }
            TagEnd::Strong => {
                self.bold = self.bold.saturating_sub(1);
                self.restyle();
            }
            TagEnd::Emphasis => {
                self.italic = self.italic.saturating_sub(1);
                self.restyle();
            }
            TagEnd::Strikethrough => {
                self.strike = self.strike.saturating_sub(1);
                self.restyle();
            }
            TagEnd::Link => {
                self.links.pop();
                self.restyle();
            }
            TagEnd::Image => {
                let Some((url, alt)) = self.image.take() else {
                    return;
                };
                // in a cell or a heading, or mid-sentence, a picture reads as its alt text
                let alone = match &self.open {
                    None => true,
                    Some((Open::Paragraph, i)) => i.text.trim().is_empty(),
                    Some(_) => false,
                };
                if !alone {
                    self.text(&alt, false);
                    return;
                }
                self.open = None;
                self.blocks().push(Block::Image { url, alt });
            }
            _ => {}
        }
    }
}

/// `text` cut around the web addresses written into it bare, each paired with its address.
fn bare_urls(text: &str) -> Vec<(&str, Option<String>)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = ["https://", "http://"]
        .iter()
        .filter_map(|p| rest.find(p))
        .min()
    {
        let end = rest[at..]
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
            .map_or(rest.len(), |e| at + e);
        let mut url = &rest[at..end];
        while let Some(c) = url.chars().last() {
            let unmatched = url.matches(')').count() > url.matches('(').count();
            if matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | ']')
                || (c == ')' && unmatched)
            {
                url = &url[..url.len() - 1];
            } else {
                break;
            }
        }
        if url.len() <= "https://".len() {
            out.push((&rest[..at + url.len()], None));
            rest = &rest[at + url.len()..];
            continue;
        }
        out.push((&rest[..at], None));
        out.push((url, Some(url.to_string())));
        rest = &rest[at + url.len()..];
    }
    out.push((rest, None));
    out
}

/// Parses `source` into blocks. Never fails: anything it does not draw becomes plain text.
pub fn parse(source: &str) -> Vec<Block> {
    let mut b = Builder {
        stack: vec![Frame::Blocks(Vec::new())],
        ..Default::default()
    };
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in TextMergeStream::new(Parser::new_ext(source, options)) {
        match event {
            Event::Start(tag) => b.start(tag),
            Event::End(tag) => b.end(tag),
            Event::Text(t) => b.text(&t, false),
            Event::Code(t) => b.text(&t, true),
            Event::SoftBreak => b.text(" ", false),
            Event::HardBreak => b.text("\n", false),
            Event::Html(t) | Event::InlineHtml(t) => b.text(&t, false),
            Event::TaskListMarker(done) => b.text(if done { "[x] " } else { "[ ] " }, false),
            Event::Rule => {
                b.close_inline();
                b.blocks().push(Block::Rule);
            }
            _ => {}
        }
    }
    b.close_inline();
    // a reply cut off mid-stream leaves containers open: fold them back down
    while b.stack.len() > 1 {
        match b.stack.pop() {
            Some(Frame::Quote(inner) | Frame::Item(inner)) => b.blocks().extend(inner),
            Some(Frame::List { start, items }) => b.blocks().push(Block::List { start, items }),
            Some(Frame::Table { head, rows, .. }) => b.blocks().push(Block::Table { head, rows }),
            _ => {}
        }
    }
    if let Some((lang, text)) = b.code.take() {
        b.blocks().push(Block::Code { lang, text });
    }
    match b.stack.pop() {
        Some(Frame::Blocks(blocks)) => blocks,
        _ => Vec::new(),
    }
}

#[derive(Default)]
pub struct Stream {
    len: usize,
    cut: usize,
    head: Vec<Block>,
}

impl Stream {
    pub fn parse(&mut self, text: &str) -> Vec<Block> {
        if text.len() < self.len {
            *self = Self::default();
        }
        self.len = text.len();
        let cut = settled(text, self.cut);
        if cut > self.cut {
            self.head.extend(parse(&text[self.cut..cut]));
            self.cut = cut;
        }
        let mut out = self.head.clone();
        out.extend(parse(&mend::mend(&text[self.cut..])));
        out
    }
}

// a blank line ends a block for good outside a fence, before a line that can't continue one
fn settled(text: &str, from: usize) -> usize {
    let mut fenced = false;
    let mut cut = from;
    let mut at = from;
    let mut blank = false;
    for line in text[from..].split_inclusive('\n') {
        let start = at;
        at += line.len();
        if !line.ends_with('\n') {
            break;
        }
        let body = line.trim_end();
        let lead = body.trim_start();
        if lead.starts_with("```") || lead.starts_with("~~~") {
            fenced = !fenced;
        }
        if !fenced && blank && fresh(line) {
            cut = start;
        }
        blank = !fenced && body.is_empty();
    }
    cut
}

fn fresh(line: &str) -> bool {
    let Some(first) = line.chars().next() else {
        return false;
    };
    if first.is_whitespace() || matches!(first, '>' | '|' | '-' | '*' | '+' | '`' | '~') {
        return false;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    !(digits > 0 && matches!(line[digits..].chars().next(), Some('.' | ')')))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_streamed_reply_parses_the_same_as_the_whole() {
        let doc = "# Title\n\nA paragraph with **bold**\nand a soft break.\n\n- one\n- two\n\n  still two\n\n3. three\n4. four\n\n> quoted\n\n> more quote\n\n```rust\nfn a() {}\n\nfn b() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nLast words.\n";
        let mut stream = Stream::default();
        for end in (0..=doc.len()).filter(|e| doc.is_char_boundary(*e)) {
            assert_eq!(
                stream.parse(&doc[..end]),
                parse(&mend::mend(&doc[..end])),
                "at {end}: {:?}",
                &doc[..end]
            );
        }
        assert!(stream.cut > 0, "something settled");
    }

    use super::*;

    fn para(blocks: &[Block]) -> &Inline {
        match &blocks[0] {
            Block::Paragraph(i) => i,
            other => panic!("not a paragraph: {other:?}"),
        }
    }

    #[test]
    fn inline_styles_become_spans() {
        let b = parse("Run `cargo test` **now**, see [docs](https://x.dev).");
        let p = para(&b);
        assert_eq!(p.text, "Run cargo test now, see docs.");
        assert_eq!(p.spans.len(), 3);
        assert!(p.spans[0].1.code);
        assert_eq!(&p.text[p.spans[0].0.clone()], "cargo test");
        assert!(p.spans[1].1.bold);
        assert_eq!(p.spans[2].1.link.as_deref(), Some("https://x.dev"));
    }

    #[test]
    fn a_bare_address_is_a_link_without_the_full_stop_after_it() {
        let b = parse(
            "Desktop: https://github.com/x/y/runs/1. Docs (https://x.dev/a_(b)), `https://not.me`",
        );
        let p = para(&b);
        let links: Vec<_> = p
            .spans
            .iter()
            .filter_map(|(r, s)| s.link.as_deref().map(|l| (&p.text[r.clone()], l)))
            .collect();
        assert_eq!(
            links,
            [
                (
                    "https://github.com/x/y/runs/1",
                    "https://github.com/x/y/runs/1"
                ),
                ("https://x.dev/a_(b)", "https://x.dev/a_(b)"),
            ]
        );
        assert_eq!(
            p.text,
            "Desktop: https://github.com/x/y/runs/1. Docs (https://x.dev/a_(b)), https://not.me"
        );
    }

    #[test]
    fn a_picture_on_its_own_line_is_a_block() {
        let b = parse(
            "Here it is:\n\n![the tower](shots/tower.png)\n\nAnd ![inline](x.png) stays text.",
        );
        assert_eq!(para(&b).text, "Here it is:");
        assert_eq!(
            b[1],
            Block::Image {
                url: "shots/tower.png".into(),
                alt: "the tower".into()
            }
        );
        assert!(matches!(&b[2], Block::Paragraph(i) if i.text == "And inline stays text."));
    }

    #[test]
    fn lists_quotes_code_and_tables() {
        let src = "# Plan\n\n1. one\n2. two\n   - nested\n\n> quoted\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n";
        let b = parse(src);
        assert!(matches!(&b[0], Block::Heading(1, i) if i.text == "Plan"));
        let Block::List { start, items } = &b[1] else {
            panic!("{:?}", b[1])
        };
        assert_eq!(*start, Some(1));
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[1][1], Block::List { start: None, .. }));
        assert!(matches!(&b[2], Block::Quote(q) if para(q).text == "quoted"));
        assert_eq!(
            b[3],
            Block::Code {
                lang: "rust".into(),
                text: "fn main() {}".into()
            }
        );
        let Block::Table { head, rows } = &b[4] else {
            panic!()
        };
        assert_eq!(
            (head.len(), rows.len(), rows[0][1].text.as_str()),
            (2, 1, "2")
        );
        assert_eq!(b[5], Block::Rule);
    }

    #[test]
    fn a_half_streamed_reply_still_parses() {
        let b = parse("Here:\n\n```sh\nls -la");
        assert_eq!(
            b[1],
            Block::Code {
                lang: "sh".into(),
                text: "ls -la".into()
            }
        );
        let b = parse("- a\n- b **bo");
        assert!(matches!(&b[0], Block::List { items, .. } if items.len() == 2));
    }

    #[test]
    #[ignore = "a measurement: cargo test --release -p hyprspace-ui perf_ -- --ignored --nocapture"]
    fn perf_streaming_a_long_reply() {
        let para = "Some **bold** text with `code` and a [link](http://x) that runs on.\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n\n";
        let mut doc = String::new();
        while doc.len() < 64 * 1024 {
            doc.push_str(para);
        }
        let t = std::time::Instant::now();
        std::hint::black_box(parse(&doc));
        let whole = t.elapsed();
        let mut stream = Stream::default();
        let steps: Vec<usize> = (1..=200).map(|i| doc.len() * i / 200).collect();
        let t = std::time::Instant::now();
        for &end in &steps {
            let end = (end..=doc.len())
                .find(|e| doc.is_char_boundary(*e))
                .unwrap();
            std::hint::black_box(stream.parse(&doc[..end]));
        }
        let frame = t.elapsed() / steps.len() as u32;
        eprintln!("markdown: {whole:?} for 64 KB whole, {frame:?} a frame while it streams");
        assert!(frame < whole, "streaming costs as much as a full parse");
    }
}
