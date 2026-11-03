// A reply streamed in a character at a time must only ever grow on screen: no block that turns
// into another kind, goes away, or takes back text it showed.

use super::*;

fn kind(b: &Block) -> &'static str {
    match b {
        Block::Paragraph(_) => "paragraph",
        Block::Heading(..) => "heading",
        Block::Code { .. } => "code",
        Block::List { start: Some(_), .. } => "numbered list",
        Block::List { .. } => "list",
        Block::Quote(_) => "quote",
        Block::Rule => "rule",
        Block::Table { .. } => "table",
        Block::Image { .. } => "image",
    }
}

fn text(b: &Block) -> String {
    let mut out = Vec::new();
    find::texts(std::slice::from_ref(b), &mut out);
    out.join("\u{1}")
}

const DOCS: &[&str] = &[
    "This is _really_ important and __very__ bold. Use `snake_case_name` in `my_file.rs`.\n\n## The `render` fix\n\n1. First step\n\n   ```bash\n   cargo test\n   ```\n\n2. Second step with a [link](https://a.b/c_d).\n   1. nested one\n   2. nested two\n\n> - quoted item\n> - another\n\n+ plus item\n+ plus two\n\nLine one<br>line two.\n\n***\n\nDone: 100% *worked*.",
    "I checked three things:\n\n- **Build**: passes\n- **Tests**: 42 pass, 1 skipped (`flaky_net`)\n- **Lint**: clean\n\nNext I'd look at `crates/ui/src/transcript/render.rs:120`, where the fold starts.\n\nWant me to do that?",
    "Yes, stealing works with the new pads. The only parts I couldn't run are the two that need a second player.\n\n**Tested** (a temporary hook let one player act as both owner and thief, and I've deleted it):\n\n| Step | Result |\n|---|---|\n| Steal | The item leaves its pad and the thief slows from 22 to 15.4. |\n| Get hit | The item drops and speed goes back to 22. |\n\nThe console showed only the usual noise.\n\n**Not tested:** dropping the item when bumped. To show them on camera:\n\n1. Start the test from the **Test** tab, set to 2 players.\n2. Player 1 buys an item and places it on a pad.\n3. Player 2 holds E on the item (\"Steal\").\n4. Player 1 hits them with the bat.\n   Or Player 1 picks it up straight away.\n5. Steal it again.\n",
    "What changed:\n- `pages()` rounds up with `div_ceil`\n- an empty result returns one page\n- the test now expects two pages\n\nSteps:\n1. Open the file\n2. Run `cargo test`\n\nDone.",
    "## Summary\n\nThe fix is in `render.rs`.\n\n### Details\n\nSome text here.\nA second line.\n\n---\n\n> Note: this is quoted\n> across lines.\n\n```rust\nlet x = 1;\n\nlet y = 2;\n```\n\nAfter the code.",
    "Here is the plan:\n\n1. **Read** the config\n   - check `a`\n   - check `b`\n2. **Write** the result\n\n- [ ] todo one\n- [x] done two\n\nSee [the docs](https://example.com/a_b) for more, or https://x.dev/page.",
    "Short answer: yes.\nThe tool said \"ok\" and returned 3 * 4 = 12.\nTotals:\n| a | b |\n|---|---|\n| 1 | 2 |\nThat's all.",
    "# Release notes\n\nVersion 2.4 ships three changes.\n\n## Fixes\n\n1. The cache now expires after **10 minutes**.\n   - It used to keep entries forever.\n     - Old entries filled the disk.\n   - Tests cover both cases.\n10. Numbers past nine keep their place.\n11. So does `eleven`.\n\n> **Note:** the migration runs once.\n>\n> > A quote inside a quote.\n> 1. step one\n> 2. step two\n\n<details>\n<summary>Raw log</summary>\n\nline one\n</details>\n\n| Field | Type | Default |\n|:--|:-:|--:|\n| `timeout` | *int* | 30 |\n| `retries` | **int** | ~~5~~ 3 |\n\n![diagram](docs/flow.png)\n\nCost: $5 * 2 = $10, and 50% off_peak.\n\nFinal word: done. ✅ 🎉",
    "I'll do it in 3 steps:\n\n1) read the file\n2) patch it\n3) run the tests\n\n* star bullet\n* another\n\n    an indented code block\n    second line\n\nUse `a | b` in shell, and `**not bold**` inside code.\n\n~~~python\ndef f():\n    return 1\n~~~\n\nThe end.",
];

#[test]
fn a_streamed_reply_only_grows() {
    let mut wrong = Vec::new();
    for doc in DOCS {
        let mut stream = Stream::default();
        let mut prev: Vec<Block> = Vec::new();
        for end in (0..=doc.len()).filter(|e| doc.is_char_boundary(*e)) {
            let cur = stream.parse(&doc[..end]);
            let at = doc[end.saturating_sub(24)..end].replace('\n', "/");
            if cur.len() < prev.len() {
                wrong.push(format!("a block went away at ...{at}"));
            }
            for (a, b) in prev.iter().zip(&cur) {
                let (ta, tb) = (text(a), text(b));
                // a table's empty last cell ends its text with a separator
                let ta = ta.trim_end_matches(|c: char| c == '\u{1}' || c.is_whitespace());
                if kind(a) != kind(b) {
                    wrong.push(format!("{} became {} at ...{at}", kind(a), kind(b)));
                } else if !tb.starts_with(ta) {
                    wrong.push(format!("{ta:?} became {tb:?} at ...{at}"));
                }
            }
            prev = cur;
        }
        if prev != parse(doc) {
            wrong.push(format!("the whole reply parses differently: {doc:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
