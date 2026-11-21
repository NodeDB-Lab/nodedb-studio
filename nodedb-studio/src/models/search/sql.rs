//! The SQL that equals a search, for the SQL tab and Open in Query. Studio
//! opens it in an editor tab and never runs it.
//!
//! Keyword and Semantic map one to one. Hybrid maps to `rrf_score` over the
//! two sources. That function takes one constant per source, which the
//! planner turns into a vector weight: it cannot carry the source weights the
//! form can set, so the statement states what it leaves out.

use crate::models::search::{SearchMode, SearchRequest, SearchSql};

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn array(vector: &[f32]) -> String {
    let values: Vec<String> = vector.iter().map(|v| v.to_string()).collect();
    format!("ARRAY[{}]", values.join(", "))
}

/// The statement of `request`. `vector` is the query vector: the explicit
/// values, or the stored embedding of the named record.
pub fn search_sql(request: &SearchRequest, id_field: &str, vector: Option<&[f32]>) -> SearchSql {
    let text_field = request.text_field.as_deref().unwrap_or("body");
    let vector_field = request.vector_field.as_deref().unwrap_or("embedding");
    let words = quote(&request.query);
    let mut wheres: Vec<String> = Vec::new();
    if request.mode == SearchMode::Keyword {
        wheres.push(format!("text_match({text_field}, {words})"));
    }
    for f in &request.filters {
        wheres.push(format!("{} = {}", f.field, quote(&f.value)));
    }
    let build = |query_vec: &str| {
        let mut sql = String::new();
        match request.mode {
            SearchMode::Keyword => {
                sql.push_str(&format!(
                    "SELECT {id_field}, title, bm25_score({text_field}, {words}) AS score\nFROM {}\n",
                    request.collection
                ));
            }
            SearchMode::Semantic => {
                sql.push_str(&format!(
                    "SELECT {id_field}, title, vector_distance({vector_field}, {query_vec}) AS score\nFROM {}\n",
                    request.collection
                ));
            }
            SearchMode::Hybrid => {
                sql.push_str(&format!(
                    "SELECT {id_field}, title, rrf_score(\n    vector_distance({vector_field}, {query_vec}),\n    bm25_score({text_field}, {words}),\n    {k}, {k}\n) AS score\nFROM {}\n",
                    request.collection,
                    k = request.fusion.k
                ));
            }
        }
        if !wheres.is_empty() {
            sql.push_str(&format!("WHERE {}\n", wheres.join("\n  AND ")));
        }
        let order = match request.mode {
            SearchMode::Keyword | SearchMode::Hybrid => "score DESC",
            SearchMode::Semantic => "score ASC",
        };
        sql.push_str(&format!("ORDER BY {order}\nLIMIT {};", request.limit));
        sql
    };
    let display = build("$query_vec");
    let executable = match vector {
        Some(v) if request.mode.uses_vector() => build(&array(v)),
        Some(_) | None => display.clone(),
    };
    let caveat = (request.mode == SearchMode::Hybrid
        && (request.fusion.keyword_weight != request.fusion.vector_weight
            || request.fusion.candidates != crate::models::search::DEFAULT_CANDIDATES))
        .then(|| {
            "This statement does not carry the source weights or the candidate budget set in Advanced ranking. Its constants set one vector weight, so its ranks can differ from the ones shown.".to_string()
        })
        .or_else(|| {
            (request.mode == SearchMode::Hybrid).then(|| {
                "The hybrid planner turns the two constants into a vector weight, so its scores can differ from the ones shown until the server fuses with k directly.".to_string()
            })
        });
    SearchSql {
        display,
        executable,
        caveat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::Filter;
    use crate::models::search::{Fusion, VectorInput};

    fn request(mode: SearchMode) -> SearchRequest {
        SearchRequest {
            collection: "articles".into(),
            mode,
            query: "distributed database".into(),
            text_field: Some("body".into()),
            vector_field: Some("embedding".into()),
            vector: VectorInput::Values(vec![0.5, -0.25]),
            filters: vec![Filter {
                field: "language".into(),
                value: "en".into(),
            }],
            limit: 20,
            fusion: Fusion::default(),
        }
    }

    #[test]
    fn keyword_matches_in_where_and_ranks_by_bm25() {
        let s = search_sql(&request(SearchMode::Keyword), "_id", None);
        assert!(
            s.display
                .contains("bm25_score(body, 'distributed database')")
        );
        assert!(
            s.display
                .contains("text_match(body, 'distributed database')")
        );
        assert!(s.display.contains("language = 'en'"));
        assert!(s.display.ends_with("ORDER BY score DESC\nLIMIT 20;"));
        assert_eq!(s.caveat, None);
    }

    #[test]
    fn semantic_ranks_by_ascending_distance() {
        let s = search_sql(&request(SearchMode::Semantic), "_id", Some(&[0.5, -0.25]));
        assert!(s.display.contains("vector_distance(embedding, $query_vec)"));
        assert!(
            s.executable
                .contains("vector_distance(embedding, ARRAY[0.5, -0.25])")
        );
        assert!(s.display.contains("ORDER BY score ASC"));
        assert!(!s.display.contains("text_match"));
    }

    #[test]
    fn hybrid_names_the_constants_and_the_filters_before_the_limit() {
        let s = search_sql(&request(SearchMode::Hybrid), "_id", Some(&[0.5, -0.25]));
        assert!(s.display.contains("60, 60"));
        assert!(s.display.contains("rrf_score("));
        let (w, l) = (
            s.display.find("WHERE").expect("where"),
            s.display.find("LIMIT").expect("limit"),
        );
        assert!(w < l);
        assert!(s.caveat.is_some(), "the planner maps k to a weight");
    }

    #[test]
    fn unequal_weights_are_called_out() {
        let mut r = request(SearchMode::Hybrid);
        r.fusion.vector_weight = 2.0;
        let s = search_sql(&r, "_id", None);
        assert!(
            s.caveat
                .as_deref()
                .is_some_and(|c| c.contains("source weights"))
        );
    }

    #[test]
    fn a_quote_in_a_word_is_doubled() {
        let mut r = request(SearchMode::Keyword);
        r.query = "o'neil".into();
        assert!(search_sql(&r, "_id", None).display.contains("'o''neil'"));
    }
}
