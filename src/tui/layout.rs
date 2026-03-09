/// Row-counting helpers for the message pane scroll calculation.
///
/// The authoritative row count for a rendered [`Line`] is obtained through
/// [`Paragraph::line_count`] (ratatui's own API, enabled via the
/// `unstable-rendered-line-info` cargo feature).  This module provides
/// regression tests that document the failure mode of the original character-
/// count formula and verify that [`Paragraph::line_count`] produces correct
/// results for common IRC message patterns.

#[cfg(test)]
mod tests {
    use ratatui::{
        text::{Line, Span},
        style::{Color, Modifier, Style},
        widgets::{Paragraph, Wrap},
    };

    /// Helper: how many terminal rows does `line` occupy in `width` columns?
    fn row_count(line: Line, width: u16) -> usize {
        Paragraph::new(line)
            .wrap(Wrap { trim: false })
            .line_count(width)
            .max(1)
    }

    /// Helper for plain-text lines (system messages, etc.)
    fn row_count_str(text: &str, width: u16) -> usize {
        row_count(Line::from(text), width)
    }

    // -----------------------------------------------------------------------
    // Regression: the original char-count formula `ceil(chars / width)` was
    // wrong.  Word-wrap can require MORE rows because words are never split
    // mid-word (trailing space leaves unused columns at the end of each row).
    // -----------------------------------------------------------------------

    /// The concrete case that exposed the original scroll bug.
    ///
    /// "aa bbb cc" (9 chars) in a 5-col pane:
    ///   OLD formula → ceil(9/5) = 2   ← wrong
    ///   Correct (word-wrap) → 3 rows  ← verified below
    #[test]
    fn word_wrap_needs_more_rows_than_char_ceil() {
        assert_eq!(row_count_str("aa bbb cc", 5), 3);
    }

    /// Three words that each nearly fill the pane.
    ///
    /// "word1 word2 word3" (17 chars) in a 10-col pane:
    ///   OLD formula → ceil(17/10) = 2  ← wrong
    ///   Correct     → 3 rows           ← verified below
    #[test]
    fn three_words_each_near_pane_width() {
        assert_eq!(row_count_str("word1 word2 word3", 10), 3);
    }

    // -----------------------------------------------------------------------
    // IRC message format sanity checks
    // -----------------------------------------------------------------------

    #[test]
    fn short_message_fits_on_one_row() {
        assert_eq!(row_count_str("hello world", 80), 1);
    }

    #[test]
    fn system_message_short() {
        assert_eq!(row_count_str("  You joined #general", 80), 1);
    }

    #[test]
    fn chat_message_single_span_short() {
        assert_eq!(row_count_str(" <alice> hello world", 80), 1);
    }

    /// Chat messages use two styled spans: ` <nick>` and ` text`.
    /// Ratatui wraps the combined content, not each span independently.
    #[test]
    fn chat_message_two_spans_wraps_correctly() {
        let line = Line::from(vec![
            Span::styled(
                " <alice>",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" hello world this is a test of wrapping behaviour"),
        ]);
        // At width 40 the combined 58-char string wraps to 2 rows.
        assert_eq!(row_count(line.clone(), 40), 2);
        // At width 80 it fits on 1 row.
        assert_eq!(row_count(line, 80), 1);
    }

    /// Space that exactly fills a row is consumed at the wrap boundary;
    /// the next word starts at col 0, not col 1.
    #[test]
    fn inter_word_space_at_row_boundary_is_consumed() {
        // "hello" = 5 chars fills the row exactly; " world" should not push
        // "world" to a third row.
        assert_eq!(row_count_str("hello world", 5), 2);
    }

    #[test]
    fn long_word_wider_than_pane_is_char_split() {
        // "helloworld" (10 chars) in a 4-col pane → ceil(10/4) = 3 rows.
        assert_eq!(row_count_str("helloworld", 4), 3);
    }

    #[test]
    fn empty_text_one_row() {
        assert_eq!(row_count_str("", 80), 1);
    }
}
