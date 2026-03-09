/// Word-wrap row counting — matches ratatui's `Wrap { trim: false }` behaviour.
///
/// This module exists so the row-count logic can be unit-tested independently
/// of the rendering layer, and so `ui.rs` and tests share a single canonical
/// implementation.

/// Count the number of terminal rows that a string of `text` occupies when
/// rendered in a pane of `inner_width` columns with word-wrapping enabled
/// (`Wrap { trim: false }`).
///
/// The algorithm mirrors ratatui's internal `WordWrapper`:
///
/// 1. The text is tokenised into alternating runs of whitespace and
///    non-whitespace characters.
/// 2. Each token is placed on the current row if it fits (`col + len <=
///    inner_width`).
/// 3. If a token does not fit and the row is non-empty, a new row is started
///    and the token is retried from column 0.
/// 4. A token that is wider than `inner_width` itself (and begins at column 0)
///    is character-split across rows.
/// 5. Leading whitespace on a wrapped row is **not** stripped (`trim: false`).
///
/// Returns at least 1 (an empty or zero-width-pane always occupies one row).
pub fn word_wrap_line_count(text: &str, inner_width: usize) -> usize {
    if inner_width == 0 || text.is_empty() {
        return 1;
    }

    let chars: Vec<char> = text.chars().collect();
    let mut rows = 1usize;
    let mut col = 0usize; // current column within the active row
    let mut i = 0usize;

    while i < chars.len() {
        // Collect a run of characters with the same whitespace classification.
        let is_ws = chars[i].is_whitespace();
        let token_start = i;
        while i < chars.len() && chars[i].is_whitespace() == is_ws {
            i += 1;
        }
        // Display width: 1 per char (correct for ASCII / typical IRC text).
        let token_width = i - token_start;

        if col + token_width <= inner_width {
            // Token fits on the current row.
            col += token_width;
        } else if col == 0 {
            // Token is wider than the whole pane; character-split it across rows.
            let mut remaining = token_width;
            while remaining > 0 {
                let space = inner_width - col;
                let take = space.min(remaining);
                col += take;
                remaining -= take;
                if remaining > 0 {
                    rows += 1;
                    col = 0;
                }
            }
        } else {
            // Token does not fit on the current row; wrap and retry from col 0.
            rows += 1;
            col = 0;
            i = token_start; // re-process this token on the new row
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::word_wrap_line_count as wc;

    // ------------------------------------------------------------------
    // Edge cases
    // ------------------------------------------------------------------

    #[test]
    fn empty_text_is_one_row() {
        assert_eq!(wc("", 80), 1);
    }

    #[test]
    fn zero_width_pane_is_one_row() {
        assert_eq!(wc("hello", 0), 1);
    }

    #[test]
    fn short_text_fits_on_one_row() {
        assert_eq!(wc("hello world", 80), 1);
    }

    #[test]
    fn text_exactly_fills_one_row() {
        // "hello" = 5 chars, inner_width = 5  →  1 row
        assert_eq!(wc("hello", 5), 1);
    }

    // ------------------------------------------------------------------
    // Key regression: word-wrap uses MORE rows than ceil(chars / width)
    // ------------------------------------------------------------------

    /// The case that exposed the original bug.
    /// "aa bbb cc" (9 chars) in a 5-col pane:
    ///   character formula → ceil(9/5) = 2
    ///   word-wrap actual  → row1="aa " row2="bbb " row3="cc"  = 3
    #[test]
    fn word_wrap_needs_more_rows_than_char_ceil() {
        assert_eq!(wc("aa bbb cc", 5), 3);
    }

    /// Three words that each nearly fill the pane width.
    /// "word1 word2 word3" (17 chars) in a 10-col pane:
    ///   character formula → ceil(17/10) = 2
    ///   word-wrap actual  → row1="word1 " row2="word2 " row3="word3" = 3
    #[test]
    fn three_words_each_near_pane_width() {
        assert_eq!(wc("word1 word2 word3", 10), 3);
    }

    // ------------------------------------------------------------------
    // Character wrapping (single word wider than pane)
    // ------------------------------------------------------------------

    #[test]
    fn single_word_wider_than_pane_is_char_split() {
        // "helloworld" (10 chars) in a 4-col pane → 3 rows
        assert_eq!(wc("helloworld", 4), 3);
    }

    #[test]
    fn long_word_followed_by_short_word() {
        // "hellooooooo world" — "hellooooooo" (11) splits across rows;
        // " world" continues on next row.
        // row1="helloooooo"(10) row2="o world"(7) → 2 rows in width 10
        assert_eq!(wc("hellooooooo world", 10), 2);
    }

    // ------------------------------------------------------------------
    // Whitespace handling (trim: false — leading whitespace is preserved)
    // ------------------------------------------------------------------

    #[test]
    fn leading_spaces_count_toward_row_width() {
        // "  abc" (5 chars) in a 5-col pane → 1 row
        assert_eq!(wc("  abc", 5), 1);
    }

    #[test]
    fn irc_system_message_format() {
        // System messages are formatted as "  {text}" with 2 leading spaces.
        // Short message — 1 row regardless.
        assert_eq!(wc("  You joined #general", 80), 1);
    }

    #[test]
    fn irc_chat_message_format_short() {
        // Chat messages: " <nick> text" concatenated from two spans.
        assert_eq!(wc(" <alice> hello world", 80), 1);
    }

    #[test]
    fn irc_chat_message_wraps_at_word_boundary() {
        // " <alice> " (9) + "word1 word2" — total 20 chars in a 19-col pane.
        // row1=" <alice> word1 " (15) then "word2"(5) doesn't fit → row2="word2"
        // → 2 rows.
        assert_eq!(wc(" <alice> word1 word2", 19), 2);
    }

    // ------------------------------------------------------------------
    // Matches ceil(chars/width) when wrapping is clean
    // ------------------------------------------------------------------

    #[test]
    fn clean_wrap_matches_char_formula() {
        // "hello world" (11 chars) in a 10-col pane:
        //   row1="hello " (6) row2="world"(5) → 2 rows = ceil(11/10)
        assert_eq!(wc("hello world", 10), 2);
    }
}
