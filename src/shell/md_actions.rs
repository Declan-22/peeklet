//! Markdown editing commands and line-level list/quote/table helpers.

use gpui::Context;

use crate::editor::EditorState;

use super::markdown;
use super::root::RootView;

impl RootView {
    /// Wrap or unwrap the current selection (or insert empty delimiters at the
    /// cursor) with inline Markdown markers like `**`, `*`, `~~`, or `` ` ``.
    pub fn md_wrap_inline(&mut self, prefix: &str, suffix: &str, cx: &mut Context<Self>) {
        self.set_md_dropdown(None, cx);
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let p_chars = prefix.chars().count();
        let s_chars = suffix.chars().count();
        if let Some((a, b)) = ed.selection_range() {
            let selected = ed.selected_text().unwrap_or_default();
            if selected.starts_with(prefix)
                && selected.ends_with(suffix)
                && selected.len() >= prefix.len() + suffix.len()
            {
                let inner = &selected[prefix.len()..selected.len() - suffix.len()];
                let inner_chars = inner.chars().count();
                ed.replace_char_range(a, b, inner, a + inner_chars, Some(a));
            } else {
                let wrapped = format!("{prefix}{selected}{suffix}");
                let sel_chars = selected.chars().count();
                ed.replace_char_range(
                    a,
                    b,
                    &wrapped,
                    a + p_chars + sel_chars,
                    Some(a + p_chars),
                );
            }
        } else {
            let pos = ed.cursor();
            let (line_no, col) = ed.cursor_line_col();
            let line = if line_no < ed.len_lines() {
                ed.line_text(line_no)
            } else {
                String::new()
            };
            let chars: Vec<char> = line.chars().collect();
            let before_matches = col >= p_chars
                && chars[col - p_chars..col]
                    .iter()
                    .copied()
                    .eq(prefix.chars());
            let after_matches = col + s_chars <= chars.len()
                && chars[col..col + s_chars]
                    .iter()
                    .copied()
                    .eq(suffix.chars());
            if before_matches && after_matches {
                // Empty pair like `**|**`: pressing Ctrl+B again removes it.
                ed.replace_char_range(pos - p_chars, pos + s_chars, "", pos - p_chars, None);
            } else if after_matches {
                // At the closing delimiter like `**hello|**`: step outside it.
                for _ in 0..s_chars {
                    ed.move_right(false);
                }
            } else {
                let empty = format!("{prefix}{suffix}");
                ed.replace_char_range(pos, pos, &empty, pos + p_chars, None);
            }
        }
        self.finish_md_edit(cx);
    }

    /// Insert or wrap a Markdown link `[label](url)`.
    pub fn md_insert_link(&mut self, cx: &mut Context<Self>) {
        self.set_md_dropdown(None, cx);
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        if let Some((a, b)) = ed.selection_range() {
            let selected = ed.selected_text().unwrap_or_default();
            let sel_chars = selected.chars().count();
            let replacement = format!("[{selected}](url)");
            let url_start = a + 1 + sel_chars + 2;
            ed.replace_char_range(a, b, &replacement, url_start + 3, Some(url_start));
        } else {
            let pos = ed.cursor();
            ed.replace_char_range(pos, pos, "[](url)", pos + 1, None);
        }
        self.finish_md_edit(cx);
    }

    /// Toggle inline code (single line) or a fenced code block (multi-line
    /// selection or empty line).
    pub fn md_toggle_code(&mut self, cx: &mut Context<Self>) {
        self.set_md_dropdown(None, cx);
        let is_multiline = self
            .active_editor_mut()
            .and_then(|ed| ed.selected_text())
            .is_some_and(|s| s.contains('\n'));
        if is_multiline {
            self.md_wrap_inline("```\n", "\n```", cx);
        } else {
            self.md_wrap_inline("`", "`", cx);
        }
    }

    /// Set the current line's heading level (`0` = paragraph, `1..=6` = H1..H6).
    pub fn md_set_heading(&mut self, level: usize, cx: &mut Context<Self>) {
        self.set_md_dropdown(None, cx);
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let (line_no, col) = ed.cursor_line_col();
        if line_no >= ed.len_lines() {
            return;
        }
        let line_start = ed.line_start_char(line_no);
        let line = ed.line_text(line_no);
        let leading_ws = line.len() - line.trim_start().len();
        let trimmed = &line[leading_ws..];
        let existing_hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
        let existing_prefix_bytes = if (1..=6).contains(&existing_hashes)
            && (existing_hashes == trimmed.len() || trimmed.as_bytes()[existing_hashes] == b' ')
        {
            let space = usize::from(
                existing_hashes < trimmed.len() && trimmed.as_bytes()[existing_hashes] == b' ',
            );
            leading_ws + existing_hashes + space
        } else {
            leading_ws
        };
        let existing_prefix_chars = line[..existing_prefix_bytes].chars().count();
        let new_prefix = if level == 0 {
            String::new()
        } else {
            format!("{} ", "#".repeat(level.min(6)))
        };
        let new_prefix_chars = new_prefix.chars().count();
        let new_col = col
            .saturating_sub(existing_prefix_chars)
            .saturating_add(new_prefix_chars);
        ed.replace_char_range(
            line_start,
            line_start + existing_prefix_chars,
            &new_prefix,
            line_start + new_col,
            None,
        );
        self.finish_md_edit(cx);
    }

    /// Toggle a line-start prefix (`> `, `- `, or `1. `) on the current line
    /// or all lines covered by the selection.
    pub fn md_toggle_line_prefix(&mut self, prefix: &str, numbered: bool, cx: &mut Context<Self>) {
        self.set_md_dropdown(None, cx);
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let (start_line, end_line) = if let Some((a, b)) = ed.selection_range() {
            (ed.char_to_line(a), ed.char_to_line(b.saturating_sub(1).max(a)))
        } else {
            let (l, _) = ed.cursor_line_col();
            (l, l)
        };
        if start_line >= ed.len_lines() {
            return;
        }
        let end_line = end_line.min(ed.len_lines() - 1);
        let range_start = ed.line_start_char(start_line);
        let range_end = ed.line_start_char(end_line) + ed.line_char_len(end_line);

        let lines: Vec<String> = (start_line..=end_line).map(|l| ed.line_text(l)).collect();
        let strip_prefix = |s: &str| -> Option<String> {
            let trimmed = s.trim_start();
            let ws = &s[..s.len() - trimmed.len()];
            if numbered {
                let d = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
                if d > 0
                    && d + 1 < trimmed.len()
                    && trimmed.as_bytes()[d] == b'.'
                    && trimmed.as_bytes()[d + 1] == b' '
                {
                    return Some(format!("{ws}{}", &trimmed[d + 2..]));
                }
                None
            } else {
                trimmed
                    .strip_prefix(prefix)
                    .map(|rest| format!("{ws}{rest}"))
            }
        };
        let all_prefixed = lines.iter().all(|l| strip_prefix(l).is_some());
        let rebuilt: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if all_prefixed {
                    strip_prefix(l).unwrap_or_else(|| l.clone())
                } else if numbered {
                    format!("{}. {l}", i + 1)
                } else {
                    format!("{prefix}{l}")
                }
            })
            .collect();
        let replacement = rebuilt.join("\n");
        let rep_chars = replacement.chars().count();
        let new_anchor = (start_line != end_line).then_some(range_start);
        ed.replace_char_range(
            range_start,
            range_end,
            &replacement,
            range_start + rep_chars,
            new_anchor,
        );
        self.finish_md_edit(cx);
    }

    /// Toggle a Markdown task checkbox (`- [ ]` ↔ `- [x]`) on `line_no` without
    /// disturbing the current cursor position.
    pub fn md_toggle_task_at_line(&mut self, line_no: usize, cx: &mut Context<Self>) {
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        if line_no >= ed.len_lines() {
            return;
        }
        let line = ed.line_text(line_no);
        let Some((check_byte, is_checked, _)) = markdown::parse_task_line(&line) else {
            return;
        };
        let check_char_offset = line[..check_byte].chars().count();
        let abs_char = ed.line_start_char(line_no) + check_char_offset;
        let cur = ed.cursor();
        let anchor = ed.selection_range().map(|(a, b)| if cur == a { b } else { a });
        let next = if is_checked { " " } else { "x" };
        ed.replace_char_range(abs_char, abs_char + 1, next, cur, anchor);
        self.finish_md_edit(cx);
    }

    /// Toggle a Markdown task-list prefix (`- [ ] `) on the current line or
    /// selected lines. Converts plain `- ` bullets to `- [ ] `, and strips
    /// `- [ ] ` / `- [x] ` when all selected lines are already tasks.
    pub fn md_toggle_task_list(&mut self, cx: &mut Context<Self>) {
        self.set_md_dropdown(None, cx);
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let (start_line, end_line) = if let Some((a, b)) = ed.selection_range() {
            (ed.char_to_line(a), ed.char_to_line(b.saturating_sub(1).max(a)))
        } else {
            let (l, _) = ed.cursor_line_col();
            (l, l)
        };
        if start_line >= ed.len_lines() {
            return;
        }
        let end_line = end_line.min(ed.len_lines() - 1);
        let range_start = ed.line_start_char(start_line);
        let range_end = ed.line_start_char(end_line) + ed.line_char_len(end_line);

        let lines: Vec<String> = (start_line..=end_line).map(|l| ed.line_text(l)).collect();
        let all_tasks = lines
            .iter()
            .all(|l| markdown::parse_task_line(l).is_some());
        let rebuilt: Vec<String> = lines
            .iter()
            .map(|l| {
                let trimmed = l.trim_start();
                let ws = &l[..l.len() - trimmed.len()];
                if all_tasks {
                    if let Some((_, _, content_off)) = markdown::parse_task_line(l) {
                        format!("{ws}{}", &l[content_off..])
                    } else {
                        l.clone()
                    }
                } else if markdown::parse_task_line(l).is_some() {
                    l.clone()
                } else if trimmed.len() >= 2
                    && matches!(trimmed.as_bytes()[0], b'-' | b'*' | b'+')
                    && trimmed.as_bytes()[1] == b' '
                {
                    format!("{ws}- [ ] {}", &trimmed[2..])
                } else {
                    format!("{ws}- [ ] {trimmed}")
                }
            })
            .collect();
        let replacement = rebuilt.join("\n");
        let rep_chars = replacement.chars().count();
        let new_anchor = (start_line != end_line).then_some(range_start);
        ed.replace_char_range(
            range_start,
            range_end,
            &replacement,
            range_start + rep_chars,
            new_anchor,
        );
        self.finish_md_edit(cx);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ListEnterAction {
    Insert(String),
    ClearLine { start: usize, end: usize },
}

/// Compute what Enter should do on a list line:
/// - If the line has text after the bullet/number/checkbox, continue the list.
/// - If the line has only the bullet/number/checkbox with no text, clear it.
pub(super) fn list_enter_action(ed: &EditorState) -> ListEnterAction {
    let (line_no, col) = ed.cursor_line_col();
    if line_no >= ed.len_lines() {
        return ListEnterAction::Insert("\n".to_string());
    }
    let line = ed.line_text(line_no);
    let leading_ws = line.len() - line.trim_start().len();
    let ws = &line[..leading_ws];
    let trimmed = &line[leading_ws..];
    let ws_chars = ws.chars().count();

    if trimmed.len() >= 2
        && matches!(trimmed.as_bytes()[0], b'-' | b'*' | b'+')
        && trimmed.as_bytes()[1] == b' '
    {
        let marker = trimmed.as_bytes()[0] as char;
        let after = &trimmed[2..];
        let is_task = after.starts_with("[ ] ")
            || after.starts_with("[x] ")
            || after.starts_with("[X] ");
        let prefix_bytes = if is_task { 6 } else { 2 };
        let prefix_chars = ws_chars + prefix_bytes;
        if col >= prefix_chars {
            if trimmed[prefix_bytes..].trim().is_empty() {
                let start = ed.line_start_char(line_no);
                let end = start + ed.line_char_len(line_no);
                return ListEnterAction::ClearLine { start, end };
            }
            return ListEnterAction::Insert(if is_task {
                format!("\n{ws}{marker} [ ] ")
            } else {
                format!("\n{ws}{marker} ")
            });
        }
    }

    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0
        && digits + 1 < trimmed.len()
        && matches!(trimmed.as_bytes()[digits], b'.' | b')')
        && trimmed.as_bytes()[digits + 1] == b' '
    {
        let prefix_bytes = digits + 2;
        let prefix_chars = ws_chars + prefix_bytes;
        if col >= prefix_chars {
            if trimmed[prefix_bytes..].trim().is_empty() {
                let start = ed.line_start_char(line_no);
                let end = start + ed.line_char_len(line_no);
                return ListEnterAction::ClearLine { start, end };
            }
            if let Ok(num) = trimmed[..digits].parse::<usize>() {
                let delim = trimmed.as_bytes()[digits] as char;
                return ListEnterAction::Insert(format!("\n{ws}{}{delim} ", num + 1));
            }
        }
    }

    ListEnterAction::Insert("\n".to_string())
}

fn is_close_or_boundary(ch: Option<char>) -> bool {
    match ch {
        None => true,
        Some(c) => {
            c.is_whitespace()
                || matches!(
                    c,
                    ')' | ']' | '}' | ',' | '.' | ';' | ':' | '!' | '?' | '*' | '~' | '`' | '"'
                )
        }
    }
}

/// Code-editor-style auto-closing pairs and closing-delimiter skip-over.
///
/// Returns `Some(mutated_text)` when the keystroke was handled (either by
/// skipping over an existing closing delimiter — `mutated_text = false` — or
/// by inserting an auto-closed pair — `mutated_text = true`). Returns `None`
/// to fall through to normal character insertion.
pub(super) fn smart_type_char(ed: &mut EditorState, ch: char, is_markdown: bool) -> Option<bool> {
    if ed.selection_range().is_some() {
        return None;
    }
    let (line_no, col) = ed.cursor_line_col();
    if line_no >= ed.len_lines() {
        return None;
    }
    let line = ed.line_text(line_no);
    let chars: Vec<char> = line.chars().collect();
    let prev = col.checked_sub(1).and_then(|i| chars.get(i).copied());
    let prev2 = col.checked_sub(2).and_then(|i| chars.get(i).copied());
    let next = chars.get(col).copied();
    let cur = ed.cursor();

    // 1. Closing brackets: skip over matching closer right ahead of cursor.
    if matches!(ch, ')' | ']' | '}') {
        if next == Some(ch) {
            ed.move_right(false);
            return Some(false);
        }
        return None;
    }

    // 2. Opening brackets: auto-close when followed by whitespace/boundary.
    if let Some(close) = match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    } {
        if is_close_or_boundary(next) {
            let pair = format!("{ch}{close}");
            ed.replace_char_range(cur, cur, &pair, cur + 1, None);
            return Some(true);
        }
        return None;
    }

    // 3. Markdown symmetric delimiters (`*`, `~`, `` ` ``).
    if is_markdown && matches!(ch, '*' | '~' | '`') {
        // Double-delimiter upgrade: typing `*` inside empty `*|*` upgrades to
        // `**|**` (and `~|~` -> `~~|~~`). When there is content before the
        // first `*` (like `**hello*|*`), skip over the second `*` instead.
        if matches!(ch, '*' | '~') && prev == Some(ch) && next == Some(ch) {
            let is_empty_pair = match prev2 {
                None => true,
                Some(c) => c.is_whitespace() || matches!(c, '(' | '[' | '{' | '>' | '-'),
            };
            if is_empty_pair {
                let pair = format!("{ch}{ch}");
                ed.replace_char_range(cur, cur, &pair, cur + 1, None);
                return Some(true);
            }
        }
        // Skip over an existing closing delimiter right ahead of the cursor.
        if next == Some(ch) {
            ed.move_right(false);
            return Some(false);
        }
        // Auto-close when starting a new span (not right after a word char for `*`/`~`).
        let prev_is_word = prev.is_some_and(|c| c.is_alphanumeric());
        if is_close_or_boundary(next) && (ch == '`' || !prev_is_word) {
            let pair = format!("{ch}{ch}");
            ed.replace_char_range(cur, cur, &pair, cur + 1, None);
            return Some(true);
        }
        return None;
    }

    // 4. Quotes (`"`, `'`, `` ` `` in code).
    if matches!(ch, '"' | '\'' | '`') {
        if next == Some(ch) {
            ed.move_right(false);
            return Some(false);
        }
        let prev_is_word = prev.is_some_and(|c| c.is_alphanumeric());
        if is_close_or_boundary(next) && (ch != '\'' || !prev_is_word) {
            let pair = format!("{ch}{ch}");
            ed.replace_char_range(cur, cur, &pair, cur + 1, None);
            return Some(true);
        }
    }

    None
}

/// If the cursor sits directly between a matching empty pair (e.g. `**|**`,
/// `*|*`, `` `|` ``, `(|)`, `[|]`, `{|}`, `"|"`), Backspace deletes both sides.
pub(super) fn smart_backspace_pair(ed: &mut EditorState) -> bool {
    if ed.selection_range().is_some() {
        return false;
    }
    let (line_no, col) = ed.cursor_line_col();
    if col == 0 || line_no >= ed.len_lines() {
        return false;
    }
    let line = ed.line_text(line_no);
    let chars: Vec<char> = line.chars().collect();
    if col >= chars.len() {
        return false;
    }
    let is_pair = matches!(
        (chars[col - 1], chars[col]),
        ('(', ')')
            | ('[', ']')
            | ('{', '}')
            | ('"', '"')
            | ('\'', '\'')
            | ('`', '`')
            | ('*', '*')
            | ('~', '~')
    );
    if is_pair {
        let cur = ed.cursor();
        ed.replace_char_range(cur - 1, cur + 1, "", cur - 1, None);
        return true;
    }
    false
}

/// Compute whether a visual row is the first/last row of a contiguous
/// multi-line blockquote so its left bar renders as one continuous line.
pub(super) fn quote_pos_for_row(
    ed: &EditorState,
    line_no: usize,
    is_first_wrap_row: bool,
    is_last_wrap_row: bool,
) -> markdown::QuotePos {
    let prev_is_quote = line_no > 0 && markdown::is_quote_line(&ed.line_text(line_no - 1));
    let next_is_quote =
        line_no + 1 < ed.len_lines() && markdown::is_quote_line(&ed.line_text(line_no + 1));
    markdown::QuotePos {
        is_first: is_first_wrap_row && !prev_is_quote,
        is_last: is_last_wrap_row && !next_is_quote,
    }
}

/// Determine whether `line_no` is part of a Markdown table (header, separator,
/// or body row).
pub(super) fn table_row_kind(ed: &EditorState, line_no: usize) -> markdown::TableRowKind {
    let total = ed.len_lines();
    if line_no >= total {
        return markdown::TableRowKind::None;
    }
    let line = ed.line_text(line_no);
    if markdown::is_table_separator(&line) {
        if line_no > 0 && markdown::parse_table_cells(&ed.line_text(line_no - 1)).is_some() {
            return markdown::TableRowKind::Separator;
        }
        return markdown::TableRowKind::None;
    }
    if markdown::parse_table_cells(&line).is_none() {
        return markdown::TableRowKind::None;
    }
    if line_no + 1 < total && markdown::is_table_separator(&ed.line_text(line_no + 1)) {
        return markdown::TableRowKind::Header;
    }
    let mut up = line_no;
    while up > 0 {
        up -= 1;
        let prev = ed.line_text(up);
        if markdown::is_table_separator(&prev) {
            if up > 0 && markdown::parse_table_cells(&ed.line_text(up - 1)).is_some() {
                let next_is_body = line_no + 1 < total && {
                    let next = ed.line_text(line_no + 1);
                    markdown::parse_table_cells(&next).is_some()
                        && !markdown::is_table_separator(&next)
                };
                return markdown::TableRowKind::Body {
                    is_last: !next_is_body,
                };
            }
            return markdown::TableRowKind::None;
        }
        if markdown::parse_table_cells(&prev).is_none() {
            return markdown::TableRowKind::None;
        }
    }
    markdown::TableRowKind::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_continues_bulleted_numbered_and_task_lists() {
        let mut ed = EditorState::new();
        ed.set_text("- item one");
        ed.line_end(false);
        assert_eq!(list_enter_action(&ed), ListEnterAction::Insert("\n- ".into()));

        ed.set_text("- ");
        ed.line_end(false);
        assert_eq!(
            list_enter_action(&ed),
            ListEnterAction::ClearLine { start: 0, end: 2 }
        );

        ed.set_text("  3. third");
        ed.line_end(false);
        assert_eq!(
            list_enter_action(&ed),
            ListEnterAction::Insert("\n  4. ".into())
        );

        ed.set_text("  4. ");
        ed.line_end(false);
        assert_eq!(
            list_enter_action(&ed),
            ListEnterAction::ClearLine { start: 0, end: 5 }
        );

        ed.set_text("- [x] done");
        ed.line_end(false);
        assert_eq!(
            list_enter_action(&ed),
            ListEnterAction::Insert("\n- [ ] ".into())
        );

        ed.set_text("plain paragraph");
        ed.line_end(false);
        assert_eq!(list_enter_action(&ed), ListEnterAction::Insert("\n".into()));
    }

    #[test]
    fn smart_type_auto_pairs_and_skips_over_closers() {
        let mut ed = EditorState::new();
        // Simulate Ctrl+B (`**|**`) then typing `hi` then `**`.
        ed.replace_char_range(0, 0, "****", 2, None);
        ed.insert_str("hi");
        assert_eq!(ed.text_string(), "**hi**");
        assert_eq!(ed.cursor(), 4);
        assert_eq!(smart_type_char(&mut ed, '*', true), Some(false));
        assert_eq!(ed.cursor(), 5);
        assert_eq!(smart_type_char(&mut ed, '*', true), Some(false));
        assert_eq!(ed.cursor(), 6);
        assert_eq!(ed.text_string(), "**hi**");

        // Typing `**` from scratch upgrades `*|*` to `**|**`.
        let mut ed2 = EditorState::new();
        assert_eq!(smart_type_char(&mut ed2, '*', true), Some(true));
        assert_eq!(ed2.text_string(), "**");
        assert_eq!(ed2.cursor(), 1);
        assert_eq!(smart_type_char(&mut ed2, '*', true), Some(true));
        assert_eq!(ed2.text_string(), "****");
        assert_eq!(ed2.cursor(), 2);

        // Backspace inside empty pair deletes both sides.
        assert!(smart_backspace_pair(&mut ed2));
        assert_eq!(ed2.text_string(), "**");
        assert!(smart_backspace_pair(&mut ed2));
        assert_eq!(ed2.text_string(), "");

        // Brackets & inline code skip over closers.
        let mut ed3 = EditorState::new();
        assert_eq!(smart_type_char(&mut ed3, '[', true), Some(true));
        ed3.insert_str("link");
        assert_eq!(smart_type_char(&mut ed3, ']', true), Some(false));
        assert_eq!(ed3.text_string(), "[link]");
        assert_eq!(ed3.cursor(), 6);
    }
}
