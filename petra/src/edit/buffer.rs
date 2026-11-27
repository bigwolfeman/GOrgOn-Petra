//! The editor's text: chunks of UTF-8 with a line index.
//!
//! # Shape
//!
//! The text is a list of chunks, each an `Arc<String>` of at most
//! [`MAX_CHUNK`] bytes, cut on `char` boundaries. Two [`Fenwick`] trees over
//! the chunks hold their byte lengths and their `\n` counts, so "where does
//! line `n` start" and "which line holds byte `b`" are `O(log chunks)` plus a
//! scan of one chunk.
//!
//! An edit inside one chunk rewrites that chunk (copy-on-write through
//! [`Arc::make_mut`]) and updates both trees in `O(log chunks)`. An edit that
//! overflows a chunk or spans several rebuilds the chunk list near the edit
//! and the trees in `O(chunks)`; that happens at most about once per
//! [`MAX_CHUNK`]` / 2` bytes of typing, and costs microseconds at the sizes
//! the editor opens.
//!
//! # Why `Arc` chunks
//!
//! Cloning a buffer copies chunk pointers, not text. A background parse can
//! take a snapshot of a large file for the price of the chunk list and keep
//! reading it while the operator types into the live copy.
//!
//! # Units
//!
//! Every offset is a byte offset into the whole text and must sit on a
//! `char` boundary; a line is the text between two `\n`s, without the `\n`.
//! A text of `k` newlines has `k + 1` lines; the empty text has one empty
//! line.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use super::fenwick::Fenwick;

/// The most bytes one chunk holds.
pub const MAX_CHUNK: usize = 2048;

/// A chunk shorter than this merges with a neighbour when the pair fits in
/// [`MAX_CHUNK`], so deletes do not leave a long tail of tiny chunks.
const MIN_CHUNK: usize = MAX_CHUNK / 8;

/// How far back from a cut point a split looks for a `\n` to cut after, so a
/// chunk tends to end on a line end and a line tends to sit in one chunk.
const LINE_SEEK: usize = 256;

/// The text being edited. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct TextBuffer {
    chunks: Vec<Arc<String>>,
    bytes: Fenwick,
    newlines: Fenwick,
    len: usize,
    lines_total: usize,
}

impl TextBuffer {
    /// An empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A buffer holding `text`, cut into chunks near [`MAX_CHUNK`].
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        let mut chunks = Vec::with_capacity(text.len() / MAX_CHUNK + 1);
        split_into(text, MAX_CHUNK, &mut chunks);
        let mut buffer = Self {
            chunks,
            ..Self::default()
        };
        buffer.rebuild();
        buffer
    }

    /// Total bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.len
    }

    /// Whether the text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of lines: newlines plus one.
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.lines_total + 1
    }

    /// The chunks in order. Their concatenation is the text.
    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        self.chunks.iter().map(|c| c.as_str())
    }

    /// The text from byte `at` to the end of the chunk that holds it: the
    /// contiguous run a parser callback asks for. Empty at or past the end.
    #[must_use]
    pub fn chunk_at(&self, at: usize) -> &str {
        if at >= self.len {
            return "";
        }
        let (i, off) = self.locate(at);
        &self.chunks[i][off..]
    }

    /// The whole text as one string.
    #[must_use]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        let mut out = String::with_capacity(self.len);
        for c in &self.chunks {
            out.push_str(c);
        }
        out
    }

    /// The text in `range`, which must be on `char` boundaries.
    #[must_use]
    pub fn slice(&self, range: Range<usize>) -> Cow<'_, str> {
        let Range { start, end } = clamp(range, self.len);
        if start == end {
            return Cow::Borrowed("");
        }
        let (i, off) = self.locate(start);
        let first = &self.chunks[i];
        if off + (end - start) <= first.len() {
            return Cow::Borrowed(&first[off..off + (end - start)]);
        }
        let mut out = String::with_capacity(end - start);
        let mut at = start;
        let mut chunk = i;
        let mut offset = off;
        while at < end {
            let c = &self.chunks[chunk];
            let take = (c.len() - offset).min(end - at);
            out.push_str(&c[offset..offset + take]);
            at += take;
            chunk += 1;
            offset = 0;
        }
        Cow::Owned(out)
    }

    /// Byte offset where 0-based line `line` starts. Past the last line,
    /// the end of the text.
    #[must_use]
    pub fn line_start(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        if line > self.lines_total {
            return self.len;
        }
        // The chunk holding the `line`-th newline (1-based).
        let i = self.newlines.find(line - 1);
        let before = self.newlines.prefix(i);
        let mut need = line - before;
        let chunk = self.chunks[i].as_bytes();
        for (pos, b) in chunk.iter().enumerate() {
            if *b == b'\n' {
                need -= 1;
                if need == 0 {
                    return self.bytes.prefix(i) + pos + 1;
                }
            }
        }
        unreachable!("the newline index points at a chunk holding the newline")
    }

    /// Byte offset where 0-based line `line` ends, before its `\n`.
    #[must_use]
    pub fn line_end(&self, line: usize) -> usize {
        if line >= self.lines_total {
            self.len
        } else {
            self.line_start(line + 1) - 1
        }
    }

    /// The byte range of line `line`, without its `\n`.
    #[must_use]
    pub fn line_range(&self, line: usize) -> Range<usize> {
        self.line_start(line)..self.line_end(line)
    }

    /// Line `line`'s text without its `\n`, borrowed when it sits in one
    /// chunk.
    #[must_use]
    pub fn line(&self, line: usize) -> Cow<'_, str> {
        self.slice(self.line_range(line))
    }

    /// The 0-based line that holds byte `at`. The end of the text is on the
    /// last line.
    #[must_use]
    pub fn line_of(&self, at: usize) -> usize {
        if self.len == 0 {
            return 0;
        }
        let at = at.min(self.len);
        let (i, off) = if at == self.len {
            let last = self.chunks.len() - 1;
            (last, self.chunks[last].len())
        } else {
            self.locate(at)
        };
        self.newlines.prefix(i) + count_newlines(&self.chunks[i].as_bytes()[..off])
    }

    /// Line and byte column of `at`.
    #[must_use]
    pub fn point_of(&self, at: usize) -> (usize, usize) {
        let line = self.line_of(at);
        (line, at.min(self.len) - self.line_start(line))
    }

    /// The `char` that starts at byte `at`, if any.
    #[must_use]
    pub fn char_at(&self, at: usize) -> Option<char> {
        self.chunk_at(at).chars().next()
    }

    /// The `char` that ends at byte `at`, if any.
    #[must_use]
    pub fn char_before(&self, at: usize) -> Option<char> {
        if at == 0 || at > self.len {
            return None;
        }
        let (i, off) = if at == self.len {
            let last = self.chunks.len() - 1;
            (last, self.chunks[last].len())
        } else {
            self.locate(at)
        };
        if off > 0 {
            self.chunks[i][..off].chars().next_back()
        } else {
            // `locate` never returns an empty chunk, and a boundary at
            // offset 0 means the char is the previous chunk's last.
            self.chunks[i - 1].chars().next_back()
        }
    }

    /// The `char` boundary after `at`, or the end.
    #[must_use]
    pub fn next_boundary(&self, at: usize) -> usize {
        self.char_at(at).map_or(self.len, |c| at + c.len_utf8())
    }

    /// The `char` boundary before `at`, or 0.
    #[must_use]
    pub fn prev_boundary(&self, at: usize) -> usize {
        self.char_before(at).map_or(0, |c| at - c.len_utf8())
    }

    /// The largest `char` boundary at or below `at`.
    #[must_use]
    pub fn floor_boundary(&self, at: usize) -> usize {
        if at >= self.len {
            return self.len;
        }
        let (i, off) = self.locate(at);
        let chunk = &self.chunks[i];
        let mut o = off;
        while !chunk.is_char_boundary(o) {
            o -= 1;
        }
        at - (off - o)
    }

    /// Insert `text` at byte `at`.
    pub fn insert(&mut self, at: usize, text: &str) {
        self.replace(at..at, text);
    }

    /// Delete the bytes in `range`.
    pub fn delete(&mut self, range: Range<usize>) {
        self.replace(range, "");
    }

    /// Replace `range` with `text`. Both ends must be `char` boundaries.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let Range { start, end } = clamp(range, self.len);
        if start == end && text.is_empty() {
            return;
        }
        if self.chunks.is_empty() {
            split_into(text, MAX_CHUNK, &mut self.chunks);
            self.rebuild();
            return;
        }
        let (i, off_s) = self.locate_edge(start);
        let (j, off_e) = self.locate_edge(end);
        if i == j {
            let chunk_len = self.chunks[i].len();
            let new_len = chunk_len - (off_e - off_s) + text.len();
            if new_len <= MAX_CHUNK && new_len >= MIN_CHUNK.min(chunk_len) && new_len > 0 {
                // The common case: typing or deleting inside one chunk.
                let removed_nl = count_newlines(&self.chunks[i].as_bytes()[off_s..off_e]);
                let added_nl = count_newlines(text.as_bytes());
                Arc::make_mut(&mut self.chunks[i]).replace_range(off_s..off_e, text);
                self.bytes
                    .add(i, text.len() as isize - (off_e - off_s) as isize);
                self.newlines
                    .add(i, added_nl as isize - removed_nl as isize);
                self.len = self.len - (end - start) + text.len();
                self.lines_total = self.lines_total - removed_nl + added_nl;
                return;
            }
        }
        // Structural: rebuild chunks i..=j (plus a small neighbour to merge
        // with) from their joined text.
        let mut lo = i;
        let mut hi = j;
        let mut joined = String::with_capacity(
            self.chunks[i].len() + self.chunks[j].len() + text.len() + MAX_CHUNK,
        );
        let head = &self.chunks[i][..off_s];
        let tail = &self.chunks[j][off_e..];
        let middle_len = head.len() + text.len() + tail.len();
        if middle_len < MIN_CHUNK {
            if hi + 1 < self.chunks.len() && middle_len + self.chunks[hi + 1].len() <= MAX_CHUNK {
                hi += 1;
            } else if lo > 0 && middle_len + self.chunks[lo - 1].len() <= MAX_CHUNK {
                lo -= 1;
            }
        }
        if lo < i {
            joined.push_str(&self.chunks[lo]);
        }
        joined.push_str(head);
        joined.push_str(text);
        joined.push_str(tail);
        if hi > j {
            joined.push_str(&self.chunks[hi]);
        }
        let mut pieces = Vec::with_capacity(joined.len() / MAX_CHUNK + 2);
        // Even pieces, so an overflow by one byte does not leave a
        // one-byte chunk behind.
        let count = joined.len().div_ceil(MAX_CHUNK).max(1);
        split_into(&joined, joined.len().div_ceil(count).max(1), &mut pieces);
        self.chunks.splice(lo..=hi, pieces);
        self.rebuild();
    }

    /// Recompute both trees and the totals from the chunk list.
    fn rebuild(&mut self) {
        self.chunks.retain(|c| !c.is_empty());
        self.bytes = Fenwick::build(self.chunks.iter().map(|c| c.len()));
        self.newlines = Fenwick::build(self.chunks.iter().map(|c| count_newlines(c.as_bytes())));
        self.len = self.bytes.prefix(self.chunks.len());
        self.lines_total = self.newlines.prefix(self.chunks.len());
    }

    /// The chunk holding byte `at` (`at < len`) and the offset inside it.
    fn locate(&self, at: usize) -> (usize, usize) {
        let i = self.bytes.find(at);
        (i, at - self.bytes.prefix(i))
    }

    /// As [`Self::locate`], but `at == len` is the end of the last chunk, so
    /// an edit at the end of the text lands in a real chunk.
    fn locate_edge(&self, at: usize) -> (usize, usize) {
        if at >= self.len {
            let last = self.chunks.len() - 1;
            (last, self.chunks[last].len())
        } else {
            self.locate(at)
        }
    }
}

impl PartialEq for TextBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && self
                .chunks()
                .flat_map(str::bytes)
                .eq(other.chunks().flat_map(str::bytes))
    }
}

impl Eq for TextBuffer {}

fn clamp(range: Range<usize>, len: usize) -> Range<usize> {
    let end = range.end.min(len);
    range.start.min(end)..end
}

/// Newlines in `bytes`. The loop is simple enough to vectorize.
#[must_use]
pub fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|b| **b == b'\n').count()
}

/// Cut `text` into chunks of at most `target` bytes (and never more than
/// [`MAX_CHUNK`]), each ending on a `char` boundary, preferring to end just
/// after a `\n` within [`LINE_SEEK`] bytes of the cut.
fn split_into(text: &str, target: usize, out: &mut Vec<Arc<String>>) {
    let target = target.clamp(1, MAX_CHUNK);
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < text.len() {
        let remaining = text.len() - start;
        if remaining <= target {
            out.push(Arc::new(text[start..].to_owned()));
            break;
        }
        let mut cut = start + target;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        let floor = cut.saturating_sub(LINE_SEEK).max(start + 1);
        if let Some(nl) = bytes[floor..cut].iter().rposition(|b| *b == b'\n') {
            cut = floor + nl + 1;
        }
        if cut <= start {
            // A single char wider than the target cannot happen for
            // targets of four bytes or more; guard anyway.
            cut = start + text[start..].chars().next().map_or(1, char::len_utf8);
        }
        out.push(Arc::new(text[start..cut].to_owned()));
        start = cut;
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_CHUNK, TextBuffer};

    /// A reference implementation of every query, on a plain `String`.
    fn line_starts(s: &str) -> Vec<usize> {
        std::iter::once(0)
            .chain(s.match_indices('\n').map(|(i, _)| i + 1))
            .collect()
    }

    fn check(buffer: &TextBuffer, model: &str) {
        assert_eq!(buffer.len_bytes(), model.len());
        assert_eq!(buffer.to_string(), model);
        let starts = line_starts(model);
        assert_eq!(buffer.len_lines(), starts.len());
        for (n, start) in starts.iter().enumerate() {
            assert_eq!(buffer.line_start(n), *start, "line_start({n})");
        }
        for (at, _) in model
            .char_indices()
            .chain(std::iter::once((model.len(), ' ')))
        {
            let line = starts.partition_point(|s| *s <= at) - 1;
            assert_eq!(buffer.line_of(at), line, "line_of({at})");
        }
        for chunk in buffer.chunks() {
            assert!(!chunk.is_empty() && chunk.len() <= MAX_CHUNK);
        }
    }

    #[test]
    fn small_edits_keep_lines_right() {
        let mut b = TextBuffer::from_str("ab\ncd\n");
        b.insert(1, "X\nY");
        check(&b, "aX\nYb\ncd\n");
        b.delete(0..4);
        check(&b, "b\ncd\n");
        b.replace(1..3, "é漢");
        check(&b, "bé漢d\n");
        assert_eq!(b.line(0), "bé漢d");
        assert_eq!(b.line(1), "");
        assert_eq!(b.char_before(3), Some('é'));
        assert_eq!(b.prev_boundary(6), 3);
        assert_eq!(b.next_boundary(3), 6);
    }

    #[test]
    fn the_empty_buffer_has_one_line() {
        let mut b = TextBuffer::new();
        assert_eq!(b.len_lines(), 1);
        assert_eq!(b.line(0), "");
        assert_eq!(b.line_of(0), 0);
        b.insert(0, "x");
        check(&b, "x");
        b.delete(0..1);
        check(&b, "");
    }

    #[test]
    fn big_inserts_and_deletes_across_chunks() {
        let line = "fn main() { let x = \"é漢\"; }\n";
        let text = line.repeat(500);
        let mut model = text.clone();
        let mut b = TextBuffer::from_str(&text);
        check(&b, &model);
        // Delete across many chunks.
        let (s, e) = (line.len() * 3 + 2, line.len() * 300 + 5);
        b.delete(s..e);
        model.replace_range(s..e, "");
        check(&b, &model);
        // Insert more than a chunk in the middle.
        let block = "z\n".repeat(MAX_CHUNK);
        b.insert(10, &block);
        model.insert_str(10, &block);
        check(&b, &model);
    }

    /// SC-006: the buffer equals a `String` after many random edits,
    /// including multi-byte text and edits across chunk edges.
    #[test]
    fn random_edits_match_a_string_model() {
        let pieces = [
            "a",
            "é",
            "漢",
            "\n",
            "xyz\n",
            "😀",
            "",
            "\n\n",
            "longer piece of text ",
        ];
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut model = "start\n".repeat(400);
        let mut b = TextBuffer::from_str(&model);
        let boundaries = |s: &str| -> Vec<usize> {
            s.char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(s.len()))
                .collect()
        };
        for step in 0..10_000 {
            let bounds = boundaries(&model);
            let a = bounds[(next() as usize) % bounds.len()];
            let span = (next() as usize) % 40;
            let bi = bounds.partition_point(|x| *x < a);
            let e =
                bounds[(bi + if next() % 3 == 0 { span * 20 } else { span }).min(bounds.len() - 1)];
            let mut text = String::new();
            for _ in 0..(next() % 4) {
                text.push_str(pieces[(next() as usize) % pieces.len()]);
            }
            if next() % 50 == 0 {
                text.push_str(&"q".repeat(MAX_CHUNK + 7));
            }
            b.replace(a..e, &text);
            model.replace_range(a..e, &text);
            if step % 500 == 0 {
                check(&b, &model);
            } else {
                assert_eq!(b.len_bytes(), model.len());
            }
        }
        check(&b, &model);
    }

    #[test]
    fn a_clone_is_a_snapshot() {
        let mut b = TextBuffer::from_str(&"abc\n".repeat(2000));
        let snapshot = b.clone();
        b.insert(5, "NEW");
        assert_eq!(snapshot.to_string(), "abc\n".repeat(2000));
        assert_ne!(snapshot, b);
    }
}
