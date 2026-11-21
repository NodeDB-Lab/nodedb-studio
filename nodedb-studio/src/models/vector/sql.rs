//! The SQL that equals a Vector Space screen, for Open in Query. Studio
//! opens it in an editor tab and never runs it.

use crate::models::vector::NeighborRequest;

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn array(vector: &[f32]) -> String {
    let values: Vec<String> = vector.iter().map(|v| v.to_string()).collect();
    format!("ARRAY[{}]", values.join(", "))
}

/// The nearest-neighbor query of `request`. `vector` is the query vector:
/// the record's stored embedding, or the pasted one. Filters and the source
/// exclusion sit in `WHERE`, so they apply before the `LIMIT` cut.
pub fn neighbors_sql(request: &NeighborRequest, id_field: &str, vector: &[f32]) -> String {
    let mut wheres: Vec<String> = Vec::new();
    if request.exclude_source
        && let crate::models::vector::QuerySource::Record(id) = &request.source
    {
        wheres.push(format!("{id_field} <> {}", quote(id)));
    }
    for f in &request.filters {
        wheres.push(format!("{} = {}", f.field, quote(&f.value)));
    }
    let tuning = request
        .ef_search
        .map(|e| format!(", ef_search => {e}"))
        .unwrap_or_default();
    let mut sql = format!(
        "-- Nearest neighbors in {}.{}. Distance ascends: lower is closer.\nSELECT {id_field}, title\nFROM {}\n",
        request.collection, request.field, request.collection
    );
    if !wheres.is_empty() {
        sql.push_str(&format!("WHERE {}\n", wheres.join("\n  AND ")));
    }
    sql.push_str(&format!(
        "ORDER BY vector_distance({}, {}{tuning})\nLIMIT {};",
        request.field,
        array(vector),
        request.top_k
    ));
    sql
}

/// The statement that reads an index's status.
pub fn index_status_sql(collection: &str, field: &str) -> String {
    format!("SHOW VECTOR INDEX status ON {collection}.{field};")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::Filter;
    use crate::models::vector::QuerySource;

    fn request() -> NeighborRequest {
        NeighborRequest {
            collection: "articles".into(),
            field: "embedding".into(),
            source: QuerySource::Record("art104".into()),
            top_k: 2,
            exclude_source: true,
            filters: vec![Filter {
                field: "language".into(),
                value: "en".into(),
            }],
            ef_search: None,
        }
    }

    #[test]
    fn exclusion_and_filters_sit_before_the_limit() {
        let sql = neighbors_sql(&request(), "_id", &[0.5, -0.25]);
        let where_at = sql.find("WHERE").expect("where");
        let limit_at = sql.find("LIMIT 2").expect("limit");
        assert!(where_at < limit_at);
        assert!(sql.contains("_id <> 'art104'"));
        assert!(sql.contains("language = 'en'"));
        assert!(sql.contains("vector_distance(embedding, ARRAY[0.5, -0.25])"));
    }

    #[test]
    fn a_quote_in_a_value_is_doubled() {
        let mut r = request();
        r.filters[0].value = "o'neil".into();
        assert!(neighbors_sql(&r, "_id", &[1.0]).contains("'o''neil'"));
    }

    #[test]
    fn the_beam_width_is_a_named_argument() {
        let mut r = request();
        r.ef_search = Some(128);
        assert!(neighbors_sql(&r, "_id", &[1.0]).contains(", ef_search => 128)"));
    }

    #[test]
    fn no_filters_and_no_exclusion_means_no_where() {
        let mut r = request();
        r.filters.clear();
        r.exclude_source = false;
        assert!(!neighbors_sql(&r, "_id", &[1.0]).contains("WHERE"));
    }

    #[test]
    fn the_index_query_names_the_field() {
        assert_eq!(
            index_status_sql("articles", "embedding"),
            "SHOW VECTOR INDEX status ON articles.embedding;"
        );
    }
}
