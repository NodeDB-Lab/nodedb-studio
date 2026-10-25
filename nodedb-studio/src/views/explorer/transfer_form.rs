//! The Data Transfer export form: its state, the request it builds, and the
//! one-line summary of what an export writes.

use crate::models::format::thousands;
use crate::models::records::Filter;
use crate::models::schema::CollectionSchema;
use crate::models::transfer::{Delimiter, ExportFormat, ExportRequest, ExportScope};

/// What the export form holds.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExportForm {
    pub(super) format: ExportFormat,
    /// Read only the rows the Explorer's filter matches. Ignored without one.
    pub(super) filtered: bool,
    /// `None` exports the collection's first four fields.
    pub(super) fields: Option<Vec<String>>,
    pub(super) header: bool,
    pub(super) delimiter: Delimiter,
}

impl Default for ExportForm {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            filtered: true,
            fields: None,
            header: true,
            delimiter: Delimiter::Comma,
        }
    }
}

impl ExportForm {
    /// The fields to export: the chosen ones, else the first four.
    pub(super) fn fields(&self, schema: &CollectionSchema) -> Vec<String> {
        self.fields.clone().unwrap_or_else(|| {
            schema
                .fields
                .iter()
                .take(4)
                .map(|f| f.name.clone())
                .collect()
        })
    }

    pub(super) fn request(
        &self,
        collection: &str,
        schema: &CollectionSchema,
        filter: Option<&Filter>,
    ) -> ExportRequest {
        ExportRequest {
            collection: collection.to_string(),
            format: self.format,
            scope: match filter {
                Some(f) if self.filtered => ExportScope::Filtered(f.clone()),
                Some(_) | None => ExportScope::All,
            },
            fields: self.fields(schema),
            header: self.header,
            delimiter: self.delimiter,
        }
    }
}

/// `Exports 842 filtered rows with 4 fields (CSV, UTF-8, comma)`.
pub(super) fn summary(request: &ExportRequest, total: u64) -> String {
    let which = match request.scope {
        ExportScope::Filtered(_) => "filtered rows",
        ExportScope::All => "rows",
    };
    let detail = if request.format.delimited() {
        format!(
            "{}, UTF-8, {}",
            request.format.label(),
            request.delimiter.word()
        )
    } else {
        request.format.label().to_string()
    };
    format!(
        "Exports {} {which} with {} fields ({detail})",
        thousands(total),
        request.fields.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transfer::ExportFormat;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    fn active() -> Filter {
        Filter {
            field: "status".into(),
            value: "active".into(),
        }
    }

    #[tokio::test]
    async fn the_default_fields_are_the_first_four() {
        let schema = MockConnectionService::ready()
            .collection_schema("users")
            .await
            .expect("schema");
        let form = ExportForm::default();
        assert_eq!(form.fields(&schema), ["id", "name", "email", "status"]);
        let request = form.request("users", &schema, Some(&active()));
        assert_eq!(request.scope, ExportScope::Filtered(active()));
    }

    #[tokio::test]
    async fn without_a_filter_the_scope_is_every_row() {
        let schema = MockConnectionService::ready()
            .collection_schema("users")
            .await
            .expect("schema");
        let request = ExportForm::default().request("users", &schema, None);
        assert_eq!(request.scope, ExportScope::All);
        let unfiltered = ExportForm {
            filtered: false,
            ..ExportForm::default()
        };
        assert_eq!(
            unfiltered.request("users", &schema, Some(&active())).scope,
            ExportScope::All
        );
    }

    #[tokio::test]
    async fn the_summary_states_scope_fields_and_format() {
        let schema = MockConnectionService::ready()
            .collection_schema("users")
            .await
            .expect("schema");
        let form = ExportForm::default();
        let r = form.request("users", &schema, Some(&active()));
        assert_eq!(
            summary(&r, 842),
            "Exports 842 filtered rows with 4 fields (CSV, UTF-8, comma)"
        );
        let json = ExportForm {
            format: ExportFormat::Jsonl,
            ..ExportForm::default()
        }
        .request("users", &schema, None);
        assert_eq!(
            summary(&json, 1284),
            "Exports 1,284 rows with 4 fields (JSONL)"
        );
    }
}
