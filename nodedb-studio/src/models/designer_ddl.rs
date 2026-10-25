//! DDL for staged changes, in the dialect NodeDB's parser accepts.
//!
//! A logical reference has no DDL: NodeDB does not enforce a foreign key, so
//! generating one would promise a check the server never makes.

use crate::models::collection::{Attached, Profile};
use crate::models::designer::{DesignCollection, DesignField, DesignIndex};
use crate::models::designer_change::{ChangeKind, DdlBlock, PendingChange, Unsupported};
use crate::models::designer_new::{Constraint, NewCollection};
use crate::models::schema::IndexKind;

/// The blocks and unsupported changes for `pending`, in staging order.
pub fn blocks(pending: &[PendingChange]) -> (Vec<DdlBlock>, Vec<Unsupported>) {
    let mut blocks = Vec::new();
    let mut unsupported = Vec::new();
    for change in pending {
        let (lines, metadata) = match &change.kind {
            ChangeKind::CreateCollection(new) => (create_collection(new), false),
            ChangeKind::CreateIndex(index) => match index_statement(index) {
                Some(lines) => (lines, false),
                None => {
                    unsupported.push(Unsupported {
                        change_id: change.id.clone(),
                        reason: format!("`{}` has a kind the Designer cannot create.", index.name),
                    });
                    continue;
                }
            },
            ChangeKind::AddReference(r) => (
                vec![
                    format!(
                        "-- {}.{} -> {}.{}",
                        r.from_collection, r.from_field, r.to_collection, r.to_field
                    ),
                    "-- Logical reference. Stored as workspace metadata.".to_string(),
                    "-- NodeDB does not enforce it, so no statement runs.".to_string(),
                ],
                true,
            ),
        };
        blocks.push(DdlBlock {
            change_id: change.id.clone(),
            lines,
            metadata,
        });
    }
    (blocks, unsupported)
}

/// One index statement. `None` for a kind the Designer does not create.
pub fn index_statement(i: &DesignIndex) -> Option<Vec<String>> {
    let field = i.fields.join(", ");
    Some(match (i.kind, i.unique) {
        (IndexKind::Secondary, false) => {
            vec![format!(
                "CREATE INDEX {} ON {}({field});",
                i.name, i.collection
            )]
        }
        (IndexKind::Secondary, true) => vec![format!(
            "CREATE UNIQUE INDEX {} ON {}({field});",
            i.name, i.collection
        )],
        (IndexKind::FullText, _) => vec![
            "CREATE FULLTEXT INDEX".to_string(),
            format!("  {} ON {}({field});", i.name, i.collection),
        ],
        (IndexKind::Vector, _) => {
            let v = i.vector?;
            vec![format!(
                "CREATE VECTOR INDEX {} ON {} METRIC {} DIM {};",
                i.name,
                i.collection,
                v.metric.ddl(),
                v.dimensions
            )]
        }
        (IndexKind::Primary | IndexKind::Spatial, _) => return None,
    })
}

/// The statements that create a collection from the form, with the indexes
/// its constraints and capabilities need.
pub fn create_collection(new: &NewCollection) -> Vec<String> {
    let checks: Vec<&str> = new
        .constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::Check { expression, .. } => Some(expression.as_str()),
            Constraint::Unique { .. } | Constraint::ForeignKey { .. } => None,
        })
        .collect();
    let fields: Vec<DesignField> = new.to_design().fields;
    let mut out = definition(&new.name, new.profile(), &fields, &checks);
    let design = new.to_design();
    for index in &design.indexes {
        if let Some(lines) = index_statement(index) {
            out.push(String::new());
            out.extend(lines);
        }
    }
    if new.attached(Attached::Graph) {
        out.push(String::new());
        out.push("-- Graph: no statement. Add edges with GRAPH INSERT EDGE.".to_string());
    }
    out
}

/// The statements that recreate an applied collection, for the Properties
/// DDL tab. Generated from the catalog read, not stored by the server.
pub fn describe(c: &DesignCollection) -> Vec<String> {
    let mut out = definition(c.name(), c.profile(), &c.fields, &[]);
    for index in &c.indexes {
        if let Some(lines) = index_statement(index) {
            out.push(String::new());
            out.extend(lines);
        }
    }
    out
}

fn column(f: &DesignField, strict: bool) -> String {
    let mut s = format!("{} {}", f.name(), f.ty().ddl());
    if f.primary {
        s.push_str(" PRIMARY KEY");
    } else if f.schema.required && strict {
        s.push_str(" NOT NULL");
    }
    if f.schema.time_key {
        s.push_str(" TIME_KEY");
    }
    s
}

fn definition(
    name: &str,
    profile: Profile,
    fields: &[DesignField],
    checks: &[&str],
) -> Vec<String> {
    let engine = match profile {
        Profile::Document => None,
        Profile::DocumentStrict => Some("document_strict"),
        Profile::KeyValue => Some("kv"),
        Profile::Columnar => Some("columnar"),
        Profile::Timeseries => Some("timeseries"),
        Profile::Spatial => Some("spatial"),
        Profile::Array => return vec![format!("-- {name}: arrays use CREATE ARRAY.")],
    };
    let Some(engine) = engine else {
        // A flexible document stores any field. A typeguard enforces the
        // declared ones.
        let mut out = vec![format!("CREATE COLLECTION {name};")];
        if !fields.is_empty() {
            out.push(format!("CREATE TYPEGUARD ON {name} ("));
            let last = fields.len() - 1;
            for (i, f) in fields.iter().enumerate() {
                let ty = f.ty().ddl().replace("TEXT", "STRING");
                let required = if f.schema.required || f.primary {
                    " REQUIRED"
                } else {
                    ""
                };
                let comma = if i == last && checks.is_empty() {
                    ""
                } else {
                    ","
                };
                out.push(format!("  {} {ty}{required}{comma}", f.name()));
            }
            let last_check = checks.len().saturating_sub(1);
            for (i, c) in checks.iter().enumerate() {
                let comma = if i == last_check { "" } else { "," };
                out.push(format!("  CHECK ({c}){comma}"));
            }
            out.push(");".to_string());
        }
        return out;
    };
    let mut lines: Vec<String> = fields.iter().map(|f| column(f, true)).collect();
    lines.extend(checks.iter().map(|c| format!("CHECK ({c})")));
    let mut out = vec![format!("CREATE COLLECTION {name} (")];
    let last = lines.len().saturating_sub(1);
    for (i, l) in lines.iter().enumerate() {
        out.push(format!("  {l}{}", if i == last { "" } else { "," }));
    }
    out.push(format!(") WITH (engine='{engine}');"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::designer::{ObjectState, VectorMetric, VectorParams};
    use crate::models::designer_new::{ConstraintKind, SchemaMode, Storage};
    use crate::models::schema::FieldType;

    fn shipments() -> NewCollection {
        let mut f = NewCollection::blank("analytics");
        f.name = "shipments".into();
        f.schema = SchemaMode::Strict;
        f.add_field("order_id", FieldType::Uuid, true, false);
        f.add_field("status", FieldType::String { max: None }, true, false);
        f.add_field("shipped_at", FieldType::Timestamp, false, false);
        f
    }

    #[test]
    fn a_strict_collection_declares_typed_columns() {
        assert_eq!(
            create_collection(&shipments()).join("\n"),
            "CREATE COLLECTION shipments (\n  id UUID PRIMARY KEY,\n  order_id UUID NOT NULL,\n  status TEXT NOT NULL,\n  shipped_at TIMESTAMP\n) WITH (engine='document_strict');"
        );
    }

    #[test]
    fn a_flexible_document_gets_a_typeguard() {
        let mut f = shipments();
        f.schema = SchemaMode::Flexible;
        let ddl = create_collection(&f).join("\n");
        assert!(ddl.starts_with("CREATE COLLECTION shipments;\nCREATE TYPEGUARD ON shipments ("));
        assert!(ddl.contains("  status STRING REQUIRED,"));
        assert!(ddl.contains("  shipped_at TIMESTAMP\n);"));
    }

    #[test]
    fn engines_follow_the_storage() {
        let mut f = shipments();
        f.storage = Storage::Columnar;
        assert!(
            create_collection(&f)
                .join("\n")
                .ends_with("WITH (engine='columnar');")
        );
        f.storage = Storage::KeyValue;
        assert!(
            create_collection(&f)
                .join("\n")
                .ends_with("WITH (engine='kv');")
        );
    }

    #[test]
    fn a_unique_constraint_creates_a_unique_index() {
        let mut f = shipments();
        f.add_constraint(ConstraintKind::Unique);
        if let Some(Constraint::Unique { fields, .. }) = f.constraints.first_mut() {
            *fields = vec!["order_id".into()];
        }
        assert!(
            create_collection(&f)
                .join("\n")
                .contains("CREATE UNIQUE INDEX idx_shipments_order_id ON shipments(order_id);")
        );
    }

    #[test]
    fn a_check_constraint_joins_the_column_list() {
        let mut f = shipments();
        f.add_constraint(ConstraintKind::Check);
        if let Some(Constraint::Check { expression, .. }) = f.constraints.first_mut() {
            *expression = "status <> ''".into();
        }
        assert!(
            create_collection(&f)
                .join("\n")
                .contains("  shipped_at TIMESTAMP,\n  CHECK (status <> '')\n)")
        );
    }

    fn index(kind: IndexKind) -> DesignIndex {
        DesignIndex {
            name: "idx_articles_body".into(),
            collection: "articles".into(),
            kind,
            unique: false,
            fields: vec!["body".into()],
            vector: Some(VectorParams {
                dimensions: 384,
                metric: VectorMetric::Cosine,
            }),
            state: ObjectState::Draft,
        }
    }

    #[test]
    fn index_ddl_matches_the_documented_statements() {
        assert_eq!(
            index_statement(&index(IndexKind::FullText))
                .map(|l| l.join("\n"))
                .as_deref(),
            Some("CREATE FULLTEXT INDEX\n  idx_articles_body ON articles(body);")
        );
        assert_eq!(
            index_statement(&index(IndexKind::Vector))
                .map(|l| l.join("\n"))
                .as_deref(),
            Some("CREATE VECTOR INDEX idx_articles_body ON articles METRIC cosine DIM 384;")
        );
        assert!(index_statement(&index(IndexKind::Spatial)).is_none());
    }

    #[test]
    fn a_reference_generates_comments_and_no_foreign_key() {
        let change = PendingChange {
            id: "chg-2".into(),
            kind: ChangeKind::AddReference(crate::models::designer::LogicalReference {
                id: String::new(),
                from_collection: "shipments".into(),
                from_field: "order_id".into(),
                to_collection: "orders".into(),
                to_field: "id".into(),
                state: ObjectState::Draft,
            }),
        };
        let (blocks, unsupported) = blocks(&[change]);
        assert!(unsupported.is_empty());
        assert!(blocks[0].metadata);
        let text = blocks[0].lines.join("\n");
        assert!(!text.to_uppercase().contains("FOREIGN KEY"));
        assert!(blocks[0].lines.iter().all(|l| l.starts_with("--")));
    }
}
