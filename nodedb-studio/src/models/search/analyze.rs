//! Analyzer testing: run text through a collection's bound analyzer and read
//! what it produced.
//!
//! The server supplies the tokens, their source spans, and the processing
//! trace. Studio never reproduces tokenization to fill a gap: a stage the
//! server did not report reads `Not reported`.

/// How the text is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputRole {
    /// A search query, analyzed as the query side of the index.
    #[default]
    Query,
    /// A document field, analyzed as it is when indexed.
    Document,
}

impl InputRole {
    pub const ALL: [InputRole; 2] = [InputRole::Query, InputRole::Document];

    pub fn label(self) -> &'static str {
        match self {
            InputRole::Query => "Treat as Query input",
            InputRole::Document => "Treat as Document input",
        }
    }
}

/// One analysis, every part explicit. Testing never changes the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeRequest {
    pub collection: String,
    pub index: String,
    pub field: String,
    pub text: String,
    pub role: InputRole,
}

/// The analyzer bound to the index, read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerProfile {
    /// `English`.
    pub name: String,
    pub binding_revision: u32,
    /// `UnicodeNFD`. `None` when the server does not report it.
    pub normalization: Option<String>,
    pub stop_words: Option<String>,
    pub stemmer: Option<String>,
}

/// The coordinates the token spans use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetUnit {
    Utf8Bytes,
}

impl OffsetUnit {
    /// The caption under the token table.
    pub fn caption(self) -> &'static str {
        match self {
            OffsetUnit::Utf8Bytes => "UTF-8 byte offsets · start inclusive · end exclusive",
        }
    }
}

/// One token of the stream, with the text it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzedToken {
    /// What the index stores: `distribut`.
    pub token: String,
    /// The source text the span covers: `Distributed`.
    pub original: String,
    /// Inclusive, in `OffsetUnit`.
    pub start: usize,
    /// Exclusive.
    pub end: usize,
}

/// One stage of the observed processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    pub stage: String,
    /// The text after the stage. `None` when the server did not report it.
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub request: AnalyzeRequest,
    pub profile: AnalyzerProfile,
    pub tokens: Vec<AnalyzedToken>,
    pub trace: Vec<TraceStep>,
    pub unit: OffsetUnit,
}

impl crate::services::async_state::IsEmpty for Analysis {
    fn is_empty(&self) -> bool {
        false
    }
}

impl Analysis {
    /// Whether every span lies on the request's text, on character
    /// boundaries, and reads back as the token's original text.
    #[cfg(test)]
    pub fn spans_match_the_text(&self) -> bool {
        self.tokens.iter().all(|t| {
            self.request
                .text
                .get(t.start..t.end)
                .is_some_and(|s| s == t.original)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_that_read_back_pass_and_others_fail() {
        let mut a = Analysis {
            request: AnalyzeRequest {
                collection: "articles".into(),
                index: "idx_articles_text".into(),
                field: "body".into(),
                text: "The Distributed Databases".into(),
                role: InputRole::Query,
            },
            profile: AnalyzerProfile {
                name: "English".into(),
                binding_revision: 7,
                normalization: None,
                stop_words: None,
                stemmer: None,
            },
            tokens: vec![AnalyzedToken {
                token: "distribut".into(),
                original: "Distributed".into(),
                start: 4,
                end: 15,
            }],
            trace: Vec::new(),
            unit: OffsetUnit::Utf8Bytes,
        };
        assert!(a.spans_match_the_text());
        a.tokens[0].end = 14;
        assert!(!a.spans_match_the_text());
    }
}
