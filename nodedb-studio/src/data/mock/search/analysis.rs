//! The analyzer test: the mock runs the text through the English analyzer
//! the index is bound to and reports each stage it observed.

use super::analyzer::analyze;
use crate::models::search_analyze::{
    Analysis, AnalyzeRequest, AnalyzedToken, AnalyzerProfile, OffsetUnit, TraceStep,
};
use crate::services::error::StudioError;

/// The longest text one test takes, in bytes.
pub const MAX_TEXT: usize = 10_000;
pub const INDEX: &str = "idx_articles_text";
pub const BINDING_REVISION: u32 = 7;

pub fn profile() -> AnalyzerProfile {
    AnalyzerProfile {
        name: "English".into(),
        binding_revision: BINDING_REVISION,
        normalization: Some("UnicodeNFD".into()),
        stop_words: Some("English".into()),
        stemmer: Some("Snowball English".into()),
    }
}

fn rejected(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

pub fn analysis(request: &AnalyzeRequest) -> Result<Analysis, StudioError> {
    if request.collection != "articles" {
        return Err(StudioError::UnknownObject {
            kind: "collection",
            id: request.collection.clone(),
        });
    }
    if request.index != INDEX {
        return Err(StudioError::UnknownObject {
            kind: "full-text index",
            id: request.index.clone(),
        });
    }
    if !matches!(request.field.as_str(), "title" | "body") {
        return Err(rejected(format!(
            "`{}` is not a field of {}.",
            request.field, request.index
        )));
    }
    if request.text.trim().is_empty() {
        return Err(rejected("Enter the text to analyze."));
    }
    if request.text.len() > MAX_TEXT {
        return Err(rejected(format!(
            "The text is {} bytes. One test takes up to {MAX_TEXT}.",
            request.text.len()
        )));
    }
    let run = analyze(&request.text);
    let tokens = run
        .tokens
        .iter()
        .map(|t| AnalyzedToken {
            token: t.stem.clone(),
            original: request
                .text
                .get(t.start..t.end)
                .unwrap_or_default()
                .to_string(),
            start: t.start,
            end: t.end,
        })
        .collect();
    let step = |stage: &str, result: &str| TraceStep {
        stage: stage.to_string(),
        result: Some(result.to_string()),
    };
    Ok(Analysis {
        request: request.clone(),
        profile: profile(),
        tokens,
        trace: vec![
            step("Normalize", &run.normalized),
            step("Lowercase", &run.lowered),
            step("Stop words", &run.kept),
            step("Stem", &run.stemmed),
        ],
        unit: OffsetUnit::Utf8Bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::search_analyze::InputRole;

    fn request(text: &str) -> AnalyzeRequest {
        AnalyzeRequest {
            collection: "articles".into(),
            index: INDEX.into(),
            field: "body".into(),
            text: text.into(),
            role: InputRole::Query,
        }
    }

    #[test]
    fn the_design_input_reports_two_tokens_and_four_stages() {
        let a = analysis(&request("The Distributed Databases")).expect("analyzes");
        let rows: Vec<(&str, &str, usize, usize)> = a
            .tokens
            .iter()
            .map(|t| (t.token.as_str(), t.original.as_str(), t.start, t.end))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("distribut", "Distributed", 4, 15),
                ("databas", "Databases", 16, 25)
            ]
        );
        let stages: Vec<(&str, &str)> = a
            .trace
            .iter()
            .map(|s| (s.stage.as_str(), s.result.as_deref().unwrap_or("")))
            .collect();
        assert_eq!(
            stages,
            vec![
                ("Normalize", "The Distributed Databases"),
                ("Lowercase", "the distributed databases"),
                ("Stop words", "distributed databases"),
                ("Stem", "distribut databas"),
            ]
        );
        assert!(a.spans_match_the_text());
    }

    #[test]
    fn unicode_spans_read_back_as_their_source() {
        let a = analysis(&request("Café Déjà vu")).expect("analyzes");
        assert!(a.spans_match_the_text());
        assert_eq!(a.tokens[0].original, "Café");
        assert_eq!((a.tokens[0].start, a.tokens[0].end), (0, 5));
    }

    #[test]
    fn an_empty_text_an_unknown_index_and_a_wrong_field_are_refused() {
        assert!(analysis(&request("  ")).is_err());
        let mut r = request("x");
        r.index = "idx_nope".into();
        assert!(matches!(
            analysis(&r),
            Err(StudioError::UnknownObject { .. })
        ));
        r = request("x");
        r.field = "language".into();
        assert!(matches!(analysis(&r), Err(StudioError::Rejected { .. })));
        assert!(analysis(&request(&"w ".repeat(MAX_TEXT))).is_err());
    }

    #[test]
    fn an_all_stop_word_text_has_no_tokens_and_still_traces() {
        let a = analysis(&request("the of a")).expect("analyzes");
        assert!(a.tokens.is_empty());
        assert_eq!(a.trace.len(), 4);
    }
}
