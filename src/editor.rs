//! Text buffer: a rope-backed text store with a character-index cursor,
//! selection, snapshot undo/redo, and a UI-agnostic word-wrap engine
//! (visual rows measured through a caller-provided width closure).
//!
//! The buffer is deliberately format-agnostic — it holds raw file bytes
//! as text. Rendering decisions (monospace, syntax colors, per-file-type
//! viewers) belong to the shell, not here.
//!
//! Undo stores whole-text snapshots (fine for small files; incremental
//! history comes later).

use ropey::Rope;

const UNDO_CAP: usize = 200;

/// Wrap visible text into byte ranges fitting `measure`, resolving each
/// candidate range's width through `width_of` (styling-aware: the caller
/// measures subranges in their own styles). Prefers breaking before spaces.
pub fn wrap_visible(
    visible: &str,
    measure: f32,
    width_of: &mut dyn FnMut(usize, usize) -> f32,
) -> Vec<(usize, usize)> {    if visible.is_empty() {
        return vec![(0, 0)];
    }
    if width_of(0, visible.len()) <= measure {
        return vec![(0, visible.len())];
    }
    // Words as (start, end) visible byte ranges (non-space runs).
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut wstart: Option<usize> = None;
    for (b, c) in visible.char_indices() {
        if c == ' ' || c == '\t' {
            if let Some(s) = wstart.take() {
                words.push((s, b));
            }
        } else if wstart.is_none() {
            wstart = Some(b);
        }
    }
    if let Some(s) = wstart {
        words.push((s, visible.len()));
    }
    if words.is_empty() {
        words.push((0, visible.len()));
    }
    let mut rows = Vec::new();
    let mut wi = 0;
    let mut row_start = 0;
    let mut row_has_word = false;
    while wi < words.len() {
        let (ws, we) = words[wi];
        // Width if the row ran to the word end (from row start).
        if width_of(row_start, we) <= measure {
            wi += 1;
            row_has_word = true;
            if wi == words.len() {
                rows.push((row_start, visible.len()));
            }
            continue;
        }
        if !row_has_word {
            // Single word overflows: force a mid-word break.
            let mut lo = row_start;
            let mut hi = we;
            while lo < hi {
                let mut mid = (lo + hi + 1) / 2;
                while mid > lo && !visible.is_char_boundary(mid) {
                    mid -= 1;
                }
                if mid <= lo {
                    break;
                }
                if width_of(row_start, mid) <= measure {
                    lo = mid;
                } else {
                    hi = mid.saturating_sub(1);
                }
            }
            if lo <= row_start {
                // Even one char overflows: force a single char of progress.
                lo = visible[row_start..]
                    .char_indices()
                    .nth(1)
                    .map(|(b, _)| row_start + b)
                    .unwrap_or(visible.len());
            }
            rows.push((row_start, lo));
            row_start = lo;
            // Same word continues on the next row.
            continue;
        }
        // Break before this word, skipping one space.
        let break_end = if ws > row_start && visible.as_bytes().get(ws - 1) == Some(&b' ') {
            ws - 1
        } else {
            ws
        };
        rows.push((row_start, break_end.max(row_start)));
        row_start = ws;
        row_has_word = false;
    }
    rows
}

/// Row mapping used by cursor navigation: the raw byte range a wrapped
/// row occupies inside its buffer line.
#[derive(Clone, Debug)]
pub struct VisualRow {
    pub buffer_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub fn wrap_line(
    buffer_line: usize,
    text: &str,
    measure: f32,
    width_of: &mut dyn FnMut(&str) -> f32,
) -> Vec<VisualRow> {
    wrap_visible(text, measure, &mut |a, b| width_of(&text[a..b]))
        .into_iter()
        .map(|(start_byte, end_byte)| VisualRow { buffer_line, start_byte, end_byte })
        .collect()
}

pub fn byte_for_x(text: &str, x: f32, width_of: &mut dyn FnMut(&str) -> f32) -> usize {
    if x <= 0.0 {
        return 0;
    }
    let mut best = 0;
    let mut best_distance = x;
    for (b, _) in text.char_indices().skip(1) {
        let distance = (width_of(&text[..b]) - x).abs();
        if distance < best_distance {
            best = b;
            best_distance = distance;
        }
    }
    let end_distance = (width_of(text) - x).abs();
    if end_distance < best_distance {
        text.len()
    } else {
        best
    }
}

pub struct EditorState {
    text: Rope,
    /// Cursor as a character index into the whole buffer (selection head).
    cursor: usize,
    /// Selection anchor (char index), if any. Head is always `cursor`.
    sel_anchor: Option<usize>,
    undo: Vec<Rope>,
    redo: Vec<Rope>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            cursor: 0,
            sel_anchor: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn len_chars(&self) -> usize {
        self.text.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.text.len_lines()
    }

    /// Raw cursor index (used by tests and future mouse handling).
    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    // ----- selection -----
    /// Normalized (min, max) selected char range, if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.sel_anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// Drop any selection (used by future mouse handling).
    #[allow(dead_code)]
    pub fn collapse(&mut self) {
        self.sel_anchor = None;
    }

    pub fn select_all(&mut self) {
        self.sel_anchor = Some(0);
        self.cursor = self.len_chars();
    }

    /// Select the word (or contiguous whitespace/punctuation run) under the
    /// current cursor position, used for double-clicking.
    pub fn select_word_at_cursor(&mut self) {
        let (line, col) = self.cursor_line_col();
        let line_start = self.text.line_to_char(line);
        let text = Self::line_string(&self.text, line);
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            self.sel_anchor = None;
            return;
        }
        let idx = col.min(chars.len().saturating_sub(1));
        let kind = |c: char| -> u8 {
            if c.is_alphanumeric() || c == '_' {
                0
            } else if c.is_whitespace() {
                1
            } else {
                2
            }
        };
        let target = kind(chars[idx]);
        let mut start = idx;
        while start > 0 && kind(chars[start - 1]) == target {
            start -= 1;
        }
        let mut end = idx + 1;
        while end < chars.len() && kind(chars[end]) == target {
            end += 1;
        }
        self.sel_anchor = Some(line_start + start);
        self.cursor = line_start + end;
    }

    /// Select the entire buffer line under the cursor (used for triple-click).
    pub fn select_line_at_cursor(&mut self) {
        let (line, _) = self.cursor_line_col();
        let line_start = self.text.line_to_char(line);
        let len = Self::line_chars(&self.text, line);
        self.sel_anchor = Some(line_start);
        self.cursor = line_start + len;
    }

    pub fn prepare_extend(&mut self) {
        if self.sel_anchor.is_none() {
            self.sel_anchor = Some(self.cursor);
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection_range()?;
        Some(self.text.slice(a..b).chars().collect())
    }

    /// Remove the selected range without committing (callers commit first).
    fn remove_range(&mut self, a: usize, b: usize) {
        self.text.remove(a..b);
        self.cursor = a;
        self.sel_anchor = None;
    }

    pub fn delete_selection(&mut self) {
        if let Some((a, b)) = self.selection_range() {
            self.commit();
            self.remove_range(a, b);
        }
    }

    // ----- editing -----

    fn commit(&mut self) {
        // Rope::clone is O(1) via structural sharing.
        self.undo.push(self.text.clone());
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.commit();
        if let Some((a, b)) = self.selection_range() {
            self.remove_range(a, b);
        }
        self.text.insert(self.cursor, s);
        self.cursor += s.chars().count();
    }

    /// Atomically replace char range `start..end` with `replacement` in one
    /// undo step, placing the cursor at `new_cursor` (and optional selection
    /// anchor at `new_anchor`).
    pub fn replace_char_range(
        &mut self,
        start: usize,
        end: usize,
        replacement: &str,
        new_cursor: usize,
        new_anchor: Option<usize>,
    ) {
        let len = self.len_chars();
        let start = start.min(len);
        let end = end.min(len).max(start);
        self.commit();
        if start < end {
            self.text.remove(start..end);
        }
        if !replacement.is_empty() {
            self.text.insert(start, replacement);
        }
        let new_len = self.len_chars();
        self.cursor = new_cursor.min(new_len);
        self.sel_anchor = new_anchor.map(|a| a.min(new_len));
    }

    /// Return the buffer line index containing a given char index.
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.text.char_to_line(char_idx.min(self.len_chars()))
    }

    pub fn backspace(&mut self) {
        if self.selection_range().is_some() {
            self.commit();
            let (a, b) = self.selection_range().unwrap();
            self.remove_range(a, b);
            return;
        }
        if self.cursor == 0 {
            return;
        }
        self.commit();
        self.cursor -= 1;
        self.text.remove(self.cursor..self.cursor + 1);
    }

    pub fn delete_forward(&mut self) {
        if self.selection_range().is_some() {
            self.commit();
            let (a, b) = self.selection_range().unwrap();
            self.remove_range(a, b);
            return;
        }
        if self.cursor >= self.len_chars() {
            return;
        }
        self.commit();
        self.text.remove(self.cursor..self.cursor + 1);
    }

    // ----- movement (extend = hold selection) -----

    pub fn move_left(&mut self, extend: bool) {
        if !extend {
            if let Some((a, _)) = self.selection_range() {
                self.cursor = a;
                self.sel_anchor = None;
                return;
            }
        } else {
            self.prepare_extend();
        }
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        if !extend {
            if let Some((_, b)) = self.selection_range() {
                self.cursor = b;
                self.sel_anchor = None;
                return;
            }
        } else {
            self.prepare_extend();
        }
        if self.cursor < self.len_chars() {
            self.cursor += 1;
        }
    }

    pub fn line_home(&mut self, extend: bool) {
        if !extend {
            self.sel_anchor = None;
        } else {
            self.prepare_extend();
        }
        let line = self.text.char_to_line(self.cursor);
        self.cursor = self.text.line_to_char(line);
    }

    pub fn line_end(&mut self, extend: bool) {
        if !extend {
            self.sel_anchor = None;
        } else {
            self.prepare_extend();
        }
        let line = self.text.char_to_line(self.cursor);
        self.cursor = self.text.line_to_char(line) + Self::line_chars(&self.text, line);
    }

    /// Place the cursor from a (buffer line, byte offset) pair, e.g. from
    /// visual-row hit testing or vertical movement.
    pub fn set_cursor_line_byte(&mut self, line: usize, byte: usize, extend: bool) {
        if !extend {
            self.sel_anchor = None;
        } else {
            self.prepare_extend();
        }
        let line = line.min(self.len_lines().saturating_sub(1));
        let text = Self::line_string(&self.text, line);
        let byte = byte.min(text.len());
        let mut col = 0;
        for (b, _) in text.char_indices() {
            if b >= byte {
                break;
            }
            col += 1;
        }
        // A byte offset past the last char boundary still counts that char.
        if byte == text.len() {
            col = text.chars().count();
        }
        self.cursor = self.text.line_to_char(line) + col;
    }

    // ----- undo / redo -----

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn restore(&mut self, snapshot: Rope) {
        self.text = snapshot;
        self.cursor = self.cursor.min(self.len_chars());
        self.sel_anchor = None;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.text.clone());
            self.restore(prev);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.text.clone());
            self.restore(next);
        }
    }

    // ----- queries for rendering -----

    /// Number of characters on a buffer line, excluding line endings.
    fn line_chars(text: &Rope, line: usize) -> usize {
        let slice = text.line(line);
        let mut len = slice.len_chars();
        while len > 0 && matches!(slice.char(len - 1), '\n' | '\r') {
            len -= 1;
        }
        len
    }

    fn line_string(text: &Rope, line: usize) -> String {
        let slice = text.line(line);
        let mut s = String::with_capacity(slice.len_bytes());
        for chunk in slice.chunks() {
            s.push_str(chunk);
        }
        while s.ends_with('\n') || s.ends_with('\r') {
            s.pop();
        }
        s
    }

    /// Plain text of a buffer line, without line endings.
    pub fn line_text(&self, line: usize) -> String {
        Self::line_string(&self.text, line)
    }

    /// Char index where a buffer line starts.
    pub fn line_start_char(&self, line: usize) -> usize {
        self.text.line_to_char(line)
    }

    /// Character count of a buffer line, excluding line endings.
    pub fn line_char_len(&self, line: usize) -> usize {
        Self::line_chars(&self.text, line)
    }

    /// (line, char-column) of the cursor.
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let line = self.text.char_to_line(self.cursor);
        (line, self.cursor - self.text.line_to_char(line))
    }

    /// (line, byte offset within the line-ending-free line) of the cursor.
    pub fn cursor_line_byte(&self) -> (usize, usize) {
        let (line, col) = self.cursor_line_col();
        let text = self.line_text(line);
        (line, Self::col_to_bytes(&text, col))
    }

    /// UTF-8 byte offset of a char column within a (line-ending-free) line.
    pub fn col_to_bytes(line_text: &str, col: usize) -> usize {
        line_text
            .char_indices()
            .nth(col)
            .map(|(b, _)| b)
            .unwrap_or(line_text.len())
    }

    /// Whitespace-separated token count. Kept for the file-info dock.
    #[allow(dead_code)]
    pub fn word_count(&self) -> usize {
        self.text.to_string().split_whitespace().count()
    }

    /// Replace the whole buffer (file open). Clears history/selection.
    pub fn set_text(&mut self, content: &str) {
        let mut rope = Rope::new();
        rope.insert(0, content);
        self.text = rope;
        self.cursor = 0;
        self.sel_anchor = None;
        self.undo.clear();
        self.redo.clear();
    }

    /// Whole buffer as a string (file save).
    pub fn text_string(&self) -> String {
        self.text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_moves_cursor_and_counts_words() {
        let mut ed = EditorState::new();
        ed.insert_str("Hello world");
        assert_eq!(ed.cursor(), 11);
        assert_eq!(ed.word_count(), 2);
        assert_eq!(ed.line_text(0), "Hello world");
    }

    #[test]
    fn arrows_cross_line_boundaries() {
        let mut ed = EditorState::new();
        ed.insert_str("ab\ncd");
        assert_eq!(ed.cursor(), 5);
        ed.move_left(false);
        ed.move_left(false);
        assert_eq!(ed.cursor(), 3);
        let (line, col) = ed.cursor_line_col();
        assert_eq!((line, col), (1, 0));
        ed.move_left(false);
        assert_eq!(ed.cursor_line_col(), (0, 2));
    }

    #[test]
    fn selection_extends_and_collapses() {
        let mut ed = EditorState::new();
        ed.insert_str("hello");
        ed.move_left(true);
        ed.move_left(true);
        assert_eq!(ed.selection_range(), Some((3, 5)));
        assert_eq!(ed.selected_text().as_deref(), Some("lo"));
        ed.move_left(false);
        assert_eq!(ed.selection_range(), None);
        assert_eq!(ed.cursor(), 3);
    }

    #[test]
    fn typing_replaces_selection_in_one_undo() {
        let mut ed = EditorState::new();
        ed.insert_str("hello");
        ed.line_home(false);
        ed.move_right(true);
        ed.move_right(true);
        ed.insert_str("HE");
        assert_eq!(ed.line_text(0), "HEllo");
        ed.undo();
        assert_eq!(ed.line_text(0), "hello");
    }

    #[test]
    fn backspace_joins_lines_and_undo_restores() {
        let mut ed = EditorState::new();
        ed.insert_str("ab\ncd");
        ed.line_home(false);
        ed.backspace();
        assert_eq!(ed.line_text(0), "abcd");
        assert!(ed.can_undo());
        ed.undo();
        assert_eq!(ed.len_lines(), 2);
        assert_eq!(ed.line_text(1), "cd");
        assert!(ed.can_redo());
        ed.redo();
        assert_eq!(ed.line_text(0), "abcd");
    }

    #[test]
    fn wrap_breaks_before_spaces() {
        let text = "aa bb cc";
        let rows = wrap_visible(text, 45.0, &mut |a, b| (b - a) as f32 * 10.0);
        assert_eq!(rows, vec![(0, 2), (3, 5), (6, 8)]);
    }

    #[test]
    fn wrap_breaks_mid_word_without_spaces() {
        let text = "abcdefgh";
        let rows = wrap_visible(text, 35.0, &mut |a, b| (b - a) as f32 * 10.0);
        assert!(rows.len() >= 2);
        assert_eq!(rows[0].0, 0);
        assert_eq!(rows.last().unwrap().1, 8);
        for pair in rows.windows(2) {
            assert!(pair[1].0 >= pair[0].1);
        }
    }

    #[test]
    fn wrap_empty_is_one_row() {
        let rows = wrap_visible("", 100.0, &mut |_, _| 0.0);
        assert_eq!(rows, vec![(0, 0)]);
    }

    #[test]
    fn byte_offsets_are_char_boundaries() {
        let mut ed = EditorState::new();
        ed.insert_str("héllo");
        let line = ed.line_text(0);
        assert_eq!(EditorState::col_to_bytes(&line, 1), 1);
        assert_eq!(EditorState::col_to_bytes(&line, 2), 3);
        assert_eq!(EditorState::col_to_bytes(&line, 99), line.len());
    }
}
