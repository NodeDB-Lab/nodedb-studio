//! The English analyzer the mock index is bound to: Unicode normalization,
//! split into words, lowercase, stop-word removal, and the Snowball stemmer.
//!
//! Offsets are UTF-8 byte offsets into the text as given, start inclusive and
//! end exclusive. Normalization works on each word after it is cut out, so a
//! span always names the source text and never the normalized copy.

use super::stemmer::stem;

/// The stop words the English analyzer removes.
const STOP_WORDS: [&str; 33] = [
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with",
];

/// Letters of the Latin-1 supplement that decompose: the letter and the
/// combining mark that NFD splits it into.
const DECOMPOSED: [(char, char, char); 56] = [
    ('À', 'A', '\u{300}'),
    ('Á', 'A', '\u{301}'),
    ('Â', 'A', '\u{302}'),
    ('Ã', 'A', '\u{303}'),
    ('Ä', 'A', '\u{308}'),
    ('Å', 'A', '\u{30A}'),
    ('Ç', 'C', '\u{327}'),
    ('È', 'E', '\u{300}'),
    ('É', 'E', '\u{301}'),
    ('Ê', 'E', '\u{302}'),
    ('Ë', 'E', '\u{308}'),
    ('Ì', 'I', '\u{300}'),
    ('Í', 'I', '\u{301}'),
    ('Î', 'I', '\u{302}'),
    ('Ï', 'I', '\u{308}'),
    ('Ñ', 'N', '\u{303}'),
    ('Ò', 'O', '\u{300}'),
    ('Ó', 'O', '\u{301}'),
    ('Ô', 'O', '\u{302}'),
    ('Õ', 'O', '\u{303}'),
    ('Ö', 'O', '\u{308}'),
    ('Ù', 'U', '\u{300}'),
    ('Ú', 'U', '\u{301}'),
    ('Û', 'U', '\u{302}'),
    ('Ü', 'U', '\u{308}'),
    ('Ý', 'Y', '\u{301}'),
    ('à', 'a', '\u{300}'),
    ('á', 'a', '\u{301}'),
    ('â', 'a', '\u{302}'),
    ('ã', 'a', '\u{303}'),
    ('ä', 'a', '\u{308}'),
    ('å', 'a', '\u{30A}'),
    ('ç', 'c', '\u{327}'),
    ('è', 'e', '\u{300}'),
    ('é', 'e', '\u{301}'),
    ('ê', 'e', '\u{302}'),
    ('ë', 'e', '\u{308}'),
    ('ì', 'i', '\u{300}'),
    ('í', 'i', '\u{301}'),
    ('î', 'i', '\u{302}'),
    ('ï', 'i', '\u{308}'),
    ('ñ', 'n', '\u{303}'),
    ('ò', 'o', '\u{300}'),
    ('ó', 'o', '\u{301}'),
    ('ô', 'o', '\u{302}'),
    ('õ', 'o', '\u{303}'),
    ('ö', 'o', '\u{308}'),
    ('ù', 'u', '\u{300}'),
    ('ú', 'u', '\u{301}'),
    ('û', 'u', '\u{302}'),
    ('ü', 'u', '\u{308}'),
    ('ý', 'y', '\u{301}'),
    ('ÿ', 'y', '\u{308}'),
    ('Ÿ', 'Y', '\u{308}'),
    ('Š', 'S', '\u{30C}'),
    ('š', 's', '\u{30C}'),
];

/// Normalization form D for the letters this analyzer knows. Every other
/// character is already in its decomposed form or has no decomposition.
pub fn nfd(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match DECOMPOSED.iter().find(|(d, _, _)| *d == c) {
            Some((_, base, mark)) => {
                out.push(*base);
                out.push(*mark);
            }
            None => out.push(c),
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// The words of `text` as `(start, end)` byte spans. An apostrophe between
/// two letters stays in the word.
pub fn words(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_word_char(chars[i].1) {
            i += 1;
            continue;
        }
        let start = chars[i].0;
        let mut j = i;
        while j < chars.len() {
            let c = chars[j].1;
            let inner_apostrophe =
                c == '\'' && j + 1 < chars.len() && is_word_char(chars[j + 1].1) && j > i;
            if is_word_char(c) || inner_apostrophe {
                j += 1;
            } else {
                break;
            }
        }
        let end = chars.get(j).map_or(text.len(), |(b, _)| *b);
        out.push((start, end));
        i = j;
    }
    out
}

/// One token the analyzer kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// What the index stores.
    pub stem: String,
    pub start: usize,
    pub end: usize,
    /// The lowercase word before stemming.
    pub word: String,
}

/// The whole run: every stage's text and the tokens that survived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub normalized: String,
    pub lowered: String,
    pub kept: String,
    pub stemmed: String,
    pub tokens: Vec<Token>,
}

pub fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

pub fn analyze(text: &str) -> Run {
    let normalized = nfd(text);
    let lowered = normalized.to_lowercase();
    let mut tokens = Vec::new();
    for (start, end) in words(text) {
        let Some(source) = text.get(start..end) else {
            continue;
        };
        let word = nfd(source).to_lowercase();
        if is_stop_word(&word) {
            continue;
        }
        tokens.push(Token {
            stem: stem(&word),
            start,
            end,
            word,
        });
    }
    let kept = tokens
        .iter()
        .map(|t| t.word.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let stemmed = tokens
        .iter()
        .map(|t| t.stem.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Run {
        normalized,
        lowered,
        kept,
        stemmed,
        tokens,
    }
}

/// The distinct stems of a query, in first-seen order.
pub fn query_terms(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in analyze(query).tokens {
        if !out.contains(&t.stem) {
            out.push(t.stem);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_input_analyzes_as_shown() {
        let run = analyze("The Distributed Databases");
        assert_eq!(run.normalized, "The Distributed Databases");
        assert_eq!(run.lowered, "the distributed databases");
        assert_eq!(run.kept, "distributed databases");
        assert_eq!(run.stemmed, "distribut databas");
        let spans: Vec<(usize, usize)> = run.tokens.iter().map(|t| (t.start, t.end)).collect();
        assert_eq!(spans, vec![(4, 15), (16, 25)]);
    }

    #[test]
    fn spans_are_utf8_bytes_of_the_source() {
        let text = "Café  naïve Databases";
        let run = analyze(text);
        for t in &run.tokens {
            assert!(text.is_char_boundary(t.start) && text.is_char_boundary(t.end));
        }
        // `Café` is five bytes: `é` takes two.
        assert_eq!((run.tokens[0].start, run.tokens[0].end), (0, 5));
        assert_eq!(&text[run.tokens[1].start..run.tokens[1].end], "naïve");
    }

    #[test]
    fn normalization_splits_an_accent_off_its_letter() {
        assert_eq!(nfd("é"), "e\u{301}");
        assert_eq!(nfd("plain"), "plain");
        // The token keeps the source span, not the longer normalized text.
        let run = analyze("É");
        assert_eq!((run.tokens[0].start, run.tokens[0].end), (0, 2));
    }

    #[test]
    fn stop_words_and_punctuation_drop_out() {
        let run = analyze("It is, in fact, the end!");
        assert_eq!(run.kept, "fact end");
        assert!(analyze("the of a").tokens.is_empty());
        assert!(analyze("  ...  ").tokens.is_empty());
    }

    #[test]
    fn an_inner_apostrophe_stays_in_the_word() {
        let run = analyze("don't 'quote'");
        let spans: Vec<(usize, usize)> = words("don't 'quote'");
        assert_eq!(spans, vec![(0, 5), (7, 12)]);
        assert_eq!(run.tokens.len(), 2);
    }

    #[test]
    fn query_terms_are_distinct_stems() {
        assert_eq!(
            query_terms("distributed distribution databases database"),
            vec!["distribut".to_string(), "databas".to_string()]
        );
    }
}
