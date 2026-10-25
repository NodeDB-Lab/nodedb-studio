//! What the server reports for a feed draft: checks, the change-event
//! schema, and a sample payload.

use crate::models::feed_draft::{CheckStatus, FeedDraft, FeedPreview, PreviewCheck, SchemaNode};
use crate::models::feed_grammar::parse_projection;
use crate::models::feeds::{ChangeOp, DeliveryKind, StartPosition};

/// Fields the mock catalog knows for any collection it previews.
const SAMPLE_FIELDS: [(&str, &str); 6] = [
    ("_id", "\"681f2c9e8b4a1d3f7c9e2a11\""),
    ("order_id", "\"A100327\""),
    ("store_id", "\"nyc\""),
    ("amount", "129.50"),
    ("status", "\"completed\""),
    ("updated_at", "\"2025-06-13T13:20:11Z\""),
];
/// Fields a feed can also read that the sample document omits.
const OTHER_FIELDS: [&str; 4] = ["currency", "region", "sold_at", "revenue"];

/// What the server reports for a draft.
pub fn preview(draft: &FeedDraft) -> FeedPreview {
    let source = draft.source.clone();
    let fields = parse_projection(&draft.projection).unwrap_or_default();
    let known = |f: &str| SAMPLE_FIELDS.iter().any(|(k, _)| *k == f) || OTHER_FIELDS.contains(&f);
    let unknown: Vec<&String> = fields.iter().filter(|f| !known(f)).collect();
    let mut checks = vec![PreviewCheck {
        label: "Permission".into(),
        status: CheckStatus::Pass,
        detail: format!("admin can create change feeds on {source}."),
    }];
    checks.push(if unknown.is_empty() {
        PreviewCheck {
            label: "Schema references".into(),
            status: CheckStatus::Pass,
            detail: if fields.is_empty() {
                "The feed delivers the whole document.".into()
            } else {
                format!("All {} projected fields exist in {source}.", fields.len())
            },
        }
    } else {
        PreviewCheck {
            label: "Schema references".into(),
            status: CheckStatus::Fail,
            detail: format!(
                "{} not in {source}.",
                unknown
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    });
    checks.push(PreviewCheck {
        label: "Filter".into(),
        status: CheckStatus::Pass,
        detail: if draft.filter.trim().is_empty() {
            "No filter: every row is captured.".into()
        } else {
            format!("`{}` is a supported filter.", draft.filter.trim())
        },
    });
    if draft.before_image {
        let kv = draft.collection() == "sessions";
        checks.push(PreviewCheck {
            label: "Before image".into(),
            status: if kv {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            detail: if kv {
                format!("{source} cannot supply before images. Update and delete events omit them.")
            } else {
                format!("{source} retains previous documents. Update and delete events carry them.")
            },
        });
    }
    checks.push(PreviewCheck {
        label: "Delivery target".into(),
        status: CheckStatus::Pass,
        detail: match draft.delivery {
            DeliveryKind::Webhook => "The server checks the endpoint on the first delivery.".into(),
            DeliveryKind::ClientSubscription => {
                "A client subscribes under the alias and pulls events.".into()
            }
        },
    });
    let snapshot = draft.start == StartPosition::SnapshotThenTail;
    checks.push(PreviewCheck {
        label: "Start position".into(),
        status: CheckStatus::Pass,
        detail: if snapshot {
            "The snapshot reads the rows that exist now, then continues with new changes.".into()
        } else {
            format!("Starts at: {}.", draft.start.label().to_lowercase())
        },
    });

    let doc_fields: Vec<(&str, &str)> = SAMPLE_FIELDS
        .iter()
        .copied()
        .filter(|(k, _)| fields.is_empty() || fields.iter().any(|f| f == k))
        .collect();
    let leaf = |name: &str, ty: &str| SchemaNode {
        name: name.into(),
        ty: ty.into(),
        value: None,
        children: Vec::new(),
    };
    let object = |name: &str, children: Vec<SchemaNode>| SchemaNode {
        name: name.into(),
        ty: "object".into(),
        value: None,
        children,
    };
    let mut doc_nodes: Vec<SchemaNode> = doc_fields
        .iter()
        .map(|(k, v)| {
            let ty = if v.starts_with('"') {
                "string"
            } else {
                "number"
            };
            leaf(k, ty)
        })
        .collect();
    if doc_nodes.is_empty() {
        doc_nodes.push(leaf("_id", "string"));
    }
    let mut schema = vec![
        leaf("operation", "string"),
        object(
            "ns",
            vec![
                SchemaNode {
                    value: Some(draft.database().to_string()),
                    ..leaf("db", "string")
                },
                SchemaNode {
                    value: Some(draft.collection().to_string()),
                    ..leaf("coll", "string")
                },
            ],
        ),
        object("documentKey", vec![leaf("_id", "string")]),
        object("fullDocument", doc_nodes.clone()),
    ];
    if draft.before_image {
        schema.push(object("fullDocumentBeforeChange", doc_nodes));
    }
    if draft.operations.contains(&ChangeOp::Update) {
        schema.push(object(
            "updateDescription",
            vec![
                object("updatedFields", Vec::new()),
                leaf("removedFields", "array"),
            ],
        ));
    }
    schema.push(leaf("clusterTime", "timestamp"));
    schema.push(leaf("eventTime", "timestamp"));

    let op = draft.operations.first().map_or("insert", |o| match o {
        ChangeOp::Insert => "insert",
        ChangeOp::Update => "update",
        ChangeOp::Delete => "delete",
    });
    let body = doc_fields
        .iter()
        .map(|(k, v)| format!("    \"{k}\": {v}"))
        .collect::<Vec<_>>()
        .join(",\n");
    let sample = format!(
        "{{\n  \"operation\": \"{op}\",\n  \"ns\": {{\n    \"db\": \"{}\",\n    \"coll\": \"{}\"\n  }},\n  \"documentKey\": {{ \"_id\": \"681f2c9e8b4a1d3f7c9e2a11\" }},\n  \"fullDocument\": {{\n{body}\n  }},\n  \"clusterTime\": \"2025-06-13T13:20:11Z\",\n  \"eventTime\": \"2025-06-13T13:20:11Z\"\n}}",
        draft.database(),
        draft.collection()
    );
    FeedPreview {
        checks,
        schema,
        sample,
        checkpoints: vec!["…d81".into(), "…a07".into(), "…5c2".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_follows_the_design_draft() {
        let mut d = FeedDraft::blank("analytics.sales");
        d.projection = "_id, order_id, store_id, amount, status, updated_at".into();
        d.before_image = true;
        let p = preview(&d);
        assert!(p.can_create());
        let names: Vec<_> = p.schema.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "operation",
                "ns",
                "documentKey",
                "fullDocument",
                "fullDocumentBeforeChange",
                "updateDescription",
                "clusterTime",
                "eventTime"
            ]
        );
        assert!(p.sample.contains("\"order_id\": \"A100327\""));
        assert!(!p.sample.contains("currency"));
    }

    #[test]
    fn an_unknown_projected_field_fails_the_preview() {
        let mut d = FeedDraft::blank("analytics.sales");
        d.projection = "_id, nope".into();
        let p = preview(&d);
        assert!(!p.can_create());
        assert!(
            p.checks
                .iter()
                .any(|c| c.status == CheckStatus::Fail && c.detail.contains("`nope`"))
        );
    }
}
