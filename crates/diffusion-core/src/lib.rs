//! Diffusion's GUI-independent, read-only comparison engine.
use similar::{Algorithm, ChangeTag, DiffTag, TextDiff};
use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    ops::Range,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::{Duration, Instant},
};
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};

pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_INLINE_BYTES: usize = 4096;

#[derive(Debug)]
pub struct Document {
    pub path: PathBuf,
    pub text: String,
    pub lines: Vec<Range<usize>>,
    pub line_ending: &'static str,
}
impl Document {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Choose a regular text file, not a folder or device.".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("This first version supports text files up to 16 MiB each.".into());
        }
        if bytes.contains(&0) {
            return Err(format!(
                "{} appears to be binary or UTF-16. Choose a UTF-8 text file.",
                path.display()
            ));
        }
        let text = String::from_utf8(bytes).map_err(|_| {
            "Unsupported encoding. Save the file as UTF-8 and try again.".to_owned()
        })?;
        if text.bytes().filter(|b| *b == b'\n').count() > 250_000 {
            return Err("This first version supports up to 250,000 lines per file.".into());
        }
        Ok(Self::from_text(path.to_owned(), text))
    }
    pub fn from_text(path: PathBuf, text: String) -> Self {
        let mut offset = 0;
        let lines = text
            .split_inclusive('\n')
            .map(|line| {
                let start = offset;
                offset += line.len();
                start..offset
            })
            .collect();
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count();
        let line_ending = if crlf > 0 && crlf == lf {
            "CRLF"
        } else if crlf > 0 {
            "Mixed"
        } else {
            "LF"
        };
        Self {
            path,
            text,
            lines,
            line_ending,
        }
    }
    pub fn line(&self, index: usize) -> &str {
        &self.text[self.lines[index].clone()]
    }
    pub fn display_line(&self, index: usize) -> &str {
        self.line(index).trim_end_matches(['\r', '\n'])
    }
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Strategy {
    Myers,
    #[default]
    Patience,
    Histogram,
}
#[derive(Clone, Debug, Default)]
pub struct DiffOptions {
    pub strategy: Strategy,
    pub ignore_whitespace: bool,
    pub ignore_case: bool,
    pub ignore_line_endings: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeType {
    Equal,
    Added,
    Removed,
    Modified,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineChange {
    pub bytes: Range<usize>,
}
#[derive(Debug)]
pub struct DiffLine {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: ChangeType,
    pub hunk: Option<usize>,
    pub left_inline: Vec<InlineChange>,
    pub right_inline: Vec<InlineChange>,
}
#[derive(Debug)]
pub struct DiffHunk {
    pub left_range: Range<usize>,
    pub right_range: Range<usize>,
    pub rows: Range<usize>,
    pub change_type: ChangeType,
}
#[derive(Debug, Default)]
pub struct DiffResult {
    pub rows: Vec<DiffLine>,
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub removals: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    Line(usize),
    Fold(Range<usize>),
}
impl DiffResult {
    pub fn display_rows(&self, collapse: bool, expanded: &HashSet<usize>) -> Vec<DisplayRow> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < self.rows.len() {
            if self.rows[i].kind != ChangeType::Equal {
                result.push(DisplayRow::Line(i));
                i += 1;
                continue;
            }
            let start = i;
            while i < self.rows.len() && self.rows[i].kind == ChangeType::Equal {
                i += 1;
            }
            if collapse && i - start > 9 && !expanded.contains(&start) {
                result.extend((start..start + 3).map(DisplayRow::Line));
                result.push(DisplayRow::Fold(start + 3..i - 3));
                result.extend((i - 3..i).map(DisplayRow::Line));
            } else {
                result.extend((start..i).map(DisplayRow::Line));
            }
        }
        result
    }
}

pub struct DiffEngine;
impl DiffEngine {
    pub fn compare(left: &Document, right: &Document, options: &DiffOptions) -> DiffResult {
        fn normalized(doc: &Document, options: &DiffOptions) -> Vec<String> {
            (0..doc.lines.len())
                .map(|i| {
                    let mut s = doc.line(i).to_owned();
                    if options.ignore_line_endings {
                        s = s.replace("\r\n", "\n");
                    }
                    if options.ignore_whitespace {
                        s = s
                            .chars()
                            .filter(|c| !c.is_whitespace() || *c == '\n' || *c == '\r')
                            .collect();
                    }
                    if options.ignore_case {
                        s = s.to_lowercase();
                    }
                    s
                })
                .collect()
        }
        let a = normalized(left, options);
        let b = normalized(right, options);
        let aa: Vec<&str> = a.iter().map(String::as_str).collect();
        let bb: Vec<&str> = b.iter().map(String::as_str).collect();
        let algorithm = match options.strategy {
            Strategy::Myers => Algorithm::Myers,
            Strategy::Patience => Algorithm::Patience,
            Strategy::Histogram => Algorithm::Histogram,
        };
        let diff = TextDiff::configure()
            .algorithm(algorithm)
            .timeout(Duration::from_secs(2))
            .diff_slices(&aa, &bb);
        let mut result = DiffResult::default();
        let inline_deadline = Instant::now() + Duration::from_millis(700);
        for op in diff.ops() {
            let (tag, old, new) = op.as_tag_tuple();
            let kind = match tag {
                DiffTag::Equal => ChangeType::Equal,
                DiffTag::Delete => ChangeType::Removed,
                DiffTag::Insert => ChangeType::Added,
                DiffTag::Replace => ChangeType::Modified,
            };
            let hunk = (tag != DiffTag::Equal).then_some(result.hunks.len());
            let start = result.rows.len();
            for offset in 0..old.len().max(new.len()) {
                let l = (offset < old.len()).then_some(old.start + offset);
                let r = (offset < new.len()).then_some(new.start + offset);
                let (li, ri) = if kind == ChangeType::Modified {
                    match (l, r) {
                        (Some(l), Some(r)) if Instant::now() < inline_deadline => {
                            inline_changes(left.display_line(l), right.display_line(r))
                        }
                        _ => (vec![], vec![]),
                    }
                } else {
                    (vec![], vec![])
                };
                result.rows.push(DiffLine {
                    left: l,
                    right: r,
                    kind,
                    hunk,
                    left_inline: li,
                    right_inline: ri,
                });
            }
            if hunk.is_some() {
                result.removals += old.len();
                result.additions += new.len();
                result.hunks.push(DiffHunk {
                    left_range: old,
                    right_range: new,
                    rows: start..result.rows.len(),
                    change_type: kind,
                });
            }
        }
        result
    }
}
fn inline_changes(left: &str, right: &str) -> (Vec<InlineChange>, Vec<InlineChange>) {
    if left.len().max(right.len()) > MAX_INLINE_BYTES {
        return (vec![], vec![]);
    }
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .timeout(Duration::from_millis(10))
        .diff_chars(left, right);
    let (mut l, mut r) = (0, 0);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for change in diff.iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                l += len;
                r += len;
            }
            ChangeTag::Delete => {
                a.push(InlineChange { bytes: l..l + len });
                l += len;
            }
            ChangeTag::Insert => {
                b.push(InlineChange { bytes: r..r + len });
                r += len;
            }
        }
    }
    (a, b)
}

#[derive(Debug)]
pub struct SyntaxSpan {
    pub bytes: Range<usize>,
    pub dark: [u8; 3],
    pub light: [u8; 3],
}
#[derive(Debug)]
pub struct Syntax {
    pub language: String,
    pub lines: Vec<Vec<SyntaxSpan>>,
    pub limited: bool,
}
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
pub fn highlight(document: &Document) -> Syntax {
    let syntax = document
        .path
        .extension()
        .and_then(|s| s.to_str())
        .and_then(|s| SYNTAXES.find_syntax_by_extension(s))
        .or_else(|| {
            document
                .text
                .lines()
                .next()
                .and_then(|s| SYNTAXES.find_syntax_by_first_line(s))
        })
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text());
    let mut result = Syntax {
        language: syntax.name.clone(),
        lines: Vec::new(),
        limited: false,
    };
    if document.text.len() > 2 * 1024 * 1024 || document.lines.iter().any(|r| r.len() > 8192) {
        result.limited = true;
        return result;
    }
    let mut dark = HighlightLines::new(syntax, &THEMES.themes["base16-eighties.dark"]);
    let mut light = HighlightLines::new(syntax, &THEMES.themes["InspiredGitHub"]);
    for i in 0..document.lines.len() {
        let line = document.line(i);
        let d = dark.highlight_line(line, &SYNTAXES).unwrap_or_default();
        let l = light.highlight_line(line, &SYNTAXES).unwrap_or_default();
        let mut spans = Vec::new();
        // Merge theme boundaries so a token never inherits the adjacent token's color.
        let (mut di, mut li, mut offset) = (0, 0, 0);
        let (mut de, mut le) = (
            d.first().map_or(0, |x| x.1.len()),
            l.first().map_or(0, |x| x.1.len()),
        );
        while di < d.len() && li < l.len() {
            let end = de.min(le);
            let dc = d[di].0.foreground;
            let lc = l[li].0.foreground;
            if end > offset {
                spans.push(SyntaxSpan {
                    bytes: offset..end,
                    dark: [dc.r, dc.g, dc.b],
                    light: [lc.r, lc.g, lc.b],
                });
            }
            offset = end;
            if de == end {
                di += 1;
                de += d.get(di).map_or(0, |x| x.1.len());
            }
            if le == end {
                li += 1;
                le += l.get(li).map_or(0, |x| x.1.len());
            }
        }
        result.lines.push(spans);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc(s: &str) -> Document {
        Document::from_text("test.rs".into(), s.into())
    }
    #[test]
    fn document_loader_rejects_binary_invalid_utf8_and_oversized_files() {
        let path = std::env::temp_dir().join(format!("diffusion-loader-{}", std::process::id()));
        std::fs::write(&path, b"binary\0bytes").unwrap();
        assert!(Document::load(&path).unwrap_err().contains("binary"));
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(Document::load(&path).unwrap_err().contains("encoding"));
        let file = File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(Document::load(&path).unwrap_err().contains("16 MiB"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn syntax_spans_cover_unicode_and_preserve_multiline_state() {
        let d = doc("/* café\ncontinued */\nlet value = 42;\n");
        let syntax = highlight(&d);
        assert_eq!(syntax.language, "Rust");
        for (i, spans) in syntax.lines.iter().enumerate() {
            let text: String = spans.iter().map(|s| &d.line(i)[s.bytes.clone()]).collect();
            assert_eq!(text, d.line(i));
        }
        assert_eq!(syntax.lines[0][0].dark, syntax.lines[1][0].dark);
        assert_ne!(syntax.lines[1][0].dark, syntax.lines[2][0].dark);
    }
    #[test]
    fn reconstruct_both_documents_for_all_algorithms() {
        let cases = [
            ("", ""),
            ("", "new\n"),
            ("old\n", ""),
            ("a\nb\nc\n", "a\nc\nd\n"),
            ("a\n", "a"),
            ("é🙂\n\r\n", "雪🙂\r\n"),
        ];
        for strategy in [Strategy::Myers, Strategy::Patience, Strategy::Histogram] {
            for (a, b) in cases {
                let (a, b) = (doc(a), doc(b));
                let result = DiffEngine::compare(
                    &a,
                    &b,
                    &DiffOptions {
                        strategy,
                        ..Default::default()
                    },
                );
                let l: String = result
                    .rows
                    .iter()
                    .filter_map(|r| r.left)
                    .map(|i| a.line(i))
                    .collect();
                let r: String = result
                    .rows
                    .iter()
                    .filter_map(|r| r.right)
                    .map(|i| b.line(i))
                    .collect();
                assert_eq!(l, a.text);
                assert_eq!(r, b.text);
                for h in &result.hunks {
                    assert!(!h.rows.is_empty());
                }
            }
        }
    }
    #[test]
    fn unicode_inline_boundaries_are_valid() {
        let (a, b) = (doc("let café = \"🙂\";\n"), doc("let café = \"雪\";\n"));
        let r = DiffEngine::compare(&a, &b, &DiffOptions::default());
        assert_eq!(
            &a.display_line(0)[r.rows[0].left_inline[0].bytes.clone()],
            "🙂"
        );
        assert_eq!(
            &b.display_line(0)[r.rows[0].right_inline[0].bytes.clone()],
            "雪"
        );
    }
    #[test]
    fn options_do_not_change_original_content() {
        let (a, b) = (doc("Hello  world\r\n"), doc("hello world\n"));
        assert!(
            !DiffEngine::compare(&a, &b, &DiffOptions::default())
                .hunks
                .is_empty()
        );
        let o = DiffOptions {
            ignore_case: true,
            ignore_whitespace: true,
            ignore_line_endings: true,
            ..Default::default()
        };
        assert!(DiffEngine::compare(&a, &b, &o).hunks.is_empty());
        assert_eq!(a.line(0), "Hello  world\r\n");
    }
    #[test]
    fn folds_expand_without_losing_rows() {
        let text = "same\n".repeat(100);
        let r = DiffEngine::compare(&doc(&text), &doc(&text), &DiffOptions::default());
        let display = r.display_rows(true, &HashSet::new());
        assert_eq!(display.len(), 7);
        assert_eq!(display[3], DisplayRow::Fold(3..97));
        assert_eq!(r.display_rows(true, &HashSet::from([0])).len(), 100);
    }
    #[test]
    fn newline_only_changes_remain_visible() {
        let r = DiffEngine::compare(&doc("x\n"), &doc("x"), &DiffOptions::default());
        assert_eq!(r.hunks.len(), 1);
        assert_eq!(r.rows[0].kind, ChangeType::Modified);
    }
    #[test]
    fn twenty_thousand_lines_keep_change_near_bottom() {
        let a: String = (0..20_000).map(|i| format!("line {i}\n")).collect();
        let b = a.replace("line 19990\n", "changed 19990\n");
        let r = DiffEngine::compare(&doc(&a), &doc(&b), &DiffOptions::default());
        assert_eq!(r.hunks.len(), 1);
        assert_eq!(r.hunks[0].left_range, 19990..19991);
        assert!(r.display_rows(true, &HashSet::new()).len() < 25);
    }
}
