//! The Insert / Edit record dialog's state.
//!
//! The JSON text is the one source of truth. The Form tab reads the text and
//! writes it back, so the two tabs never disagree. The editor holds the
//! record as it read it (`stored`), and a save names that revision: a write
//! after someone else's is a conflict, shown in the dialog with the edit
//! kept.

use nodedb_types::Value;

use crate::models::collection::Profile;
use crate::models::record_check::{FieldChange, Violation, changes, violations};
use crate::models::record_json::{JsonError, parse_object, pretty};
use crate::models::records::Record;
use crate::models::schema::{CollectionSchema, FieldType};
use crate::services::error::StudioError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Insert,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTab {
    Form,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Editing,
    /// The diff is on screen and the next step writes.
    Reviewing,
}

/// Why the last save did not land. Each kind has its own wording and its own
/// way out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    /// Someone wrote the record first.
    Conflict { expected: u64, stored: u64 },
    /// The record is gone.
    Gone,
    /// The session may not write.
    Denied(String),
    /// The server's validation refused the record.
    Rejected(String),
    /// The server could not be reached.
    Failed(String),
}

impl From<&StudioError> for EditorError {
    fn from(e: &StudioError) -> Self {
        match e {
            StudioError::RevisionConflict {
                expected, stored, ..
            } => EditorError::Conflict {
                expected: *expected,
                stored: *stored,
            },
            StudioError::RecordMissing { .. } => EditorError::Gone,
            StudioError::PermissionDenied { .. } => EditorError::Denied(e.to_string()),
            StudioError::Rejected { reason } => EditorError::Rejected(reason.clone()),
            StudioError::Connection(_)
            | StudioError::Auth(_)
            | StudioError::NotFound(_)
            | StudioError::Conflict(_)
            | StudioError::ReadOnly(_)
            | StudioError::Setup(_)
            | StudioError::Server(_)
            | StudioError::NotConnected
            | StudioError::UnexpectedColumns { .. }
            | StudioError::MalformedRows { .. }
            | StudioError::MissingUsername
            | StudioError::NameTaken(_)
            | StudioError::UnknownConnection(_)
            | StudioError::UnknownDatabase(_)
            | StudioError::QueryNameTaken(_)
            | StudioError::UnknownQuery(_)
            | StudioError::EmptyStatement
            | StudioError::Syntax { .. }
            | StudioError::MissingParameter(_)
            | StudioError::NoTransaction
            | StudioError::TransactionOpen
            | StudioError::ReadOnlyTransaction => EditorError::Failed(e.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordEditor {
    pub mode: EditorMode,
    pub collection: String,
    pub profile: Profile,
    pub schema: CollectionSchema,
    /// The record as read when the editor opened, or after Load stored
    /// revision. `None` when inserting.
    pub stored: Option<Record>,
    /// The id the record has, or will have. Empty lets the server assign it.
    pub id: String,
    pub text: String,
    pub tab: EditorTab,
    pub stage: Stage,
    pub busy: bool,
    pub error: Option<EditorError>,
    /// Vector fields stay out of the text and go back in on save.
    pub held: Vec<(String, Value)>,
}

/// `value` split into the part the text shows and the vectors it holds back.
fn split_vectors(value: &Value, schema: &CollectionSchema) -> (Value, Vec<(String, Value)>) {
    let Value::Object(map) = value else {
        return (value.clone(), Vec::new());
    };
    let mut shown = map.clone();
    let mut held = Vec::new();
    for f in &schema.fields {
        if matches!(f.ty, FieldType::Vector { .. })
            && let Some(v) = shown.remove(&f.name)
        {
            held.push((f.name.clone(), v));
        }
    }
    (Value::Object(shown), held)
}

/// The empty value of a type, for an Insert template.
fn blank(ty: FieldType) -> Value {
    match ty {
        FieldType::String { .. } | FieldType::Decimal { .. } => Value::String(String::new()),
        FieldType::Int64 => Value::Integer(0),
        FieldType::Float64 => Value::Float(0.0),
        FieldType::Bool => Value::Bool(false),
        FieldType::Object => Value::Object(Default::default()),
        FieldType::Array => Value::Array(Vec::new()),
        FieldType::Timestamp
        | FieldType::Datetime
        | FieldType::Json
        | FieldType::Vector { .. }
        | FieldType::Geometry { .. }
        | FieldType::Bytes => Value::Null,
    }
}

impl RecordEditor {
    pub fn edit(
        collection: &str,
        profile: Profile,
        schema: CollectionSchema,
        stored: Record,
    ) -> Self {
        let (shown, held) = split_vectors(&stored.value, &schema);
        Self {
            mode: EditorMode::Edit,
            collection: collection.to_string(),
            profile,
            text: pretty(&shown, &schema.fields),
            id: stored.id.clone(),
            stored: Some(stored),
            schema,
            tab: EditorTab::Json,
            stage: Stage::Editing,
            busy: false,
            error: None,
            held,
        }
    }

    /// An Insert dialog with a template of the required fields.
    pub fn insert(collection: &str, profile: Profile, schema: CollectionSchema) -> Self {
        let template: Value = Value::Object(
            schema
                .fields
                .iter()
                .filter(|f| f.required && !matches!(f.ty, FieldType::Vector { .. }))
                .map(|f| (f.name.clone(), blank(f.ty)))
                .collect(),
        );
        Self {
            mode: EditorMode::Insert,
            collection: collection.to_string(),
            profile,
            text: pretty(&template, &schema.fields),
            id: String::new(),
            stored: None,
            schema,
            tab: EditorTab::Json,
            stage: Stage::Editing,
            busy: false,
            error: None,
            held: Vec::new(),
        }
    }

    pub fn title(&self) -> String {
        match (&self.mode, &self.stored) {
            (EditorMode::Edit, Some(r)) => format!(
                "Edit record {}/{}, revision {}",
                self.collection, r.id, r.revision
            ),
            (EditorMode::Edit, None) | (EditorMode::Insert, _) => {
                format!("Insert record into {}", self.collection)
            }
        }
    }

    /// The record the text describes, vectors restored.
    pub fn draft(&self) -> Result<Value, JsonError> {
        let Value::Object(mut map) = parse_object(&self.text, &self.schema.fields)? else {
            return Err(JsonError {
                message: "A record is a JSON object: start with `{`.".to_string(),
            });
        };
        for (k, v) in &self.held {
            map.entry(k.clone()).or_insert_with(|| v.clone());
        }
        Ok(Value::Object(map))
    }

    /// The stored record as it compares: vectors held back on both sides.
    fn baseline(&self) -> Value {
        match &self.stored {
            Some(r) => r.value.clone(),
            None => Value::Object(Default::default()),
        }
    }

    /// Fields that differ from the stored record, or every field of an insert.
    pub fn changes(&self) -> Vec<FieldChange> {
        match self.draft() {
            Ok(d) => changes(&self.baseline(), &d, &self.schema.fields),
            Err(_) => Vec::new(),
        }
    }

    /// What blocks a save: a JSON error, a schema violation, a changed id.
    pub fn problems(&self) -> Vec<Violation> {
        let draft = match self.draft() {
            Ok(d) => d,
            Err(e) => {
                return vec![Violation {
                    field: String::new(),
                    message: e.message,
                }];
            }
        };
        let mut out = violations(self.profile.enforces_schema(), &self.schema.fields, &draft);
        if let (EditorMode::Edit, Some(stored), Some(id_field)) =
            (&self.mode, &self.stored, &self.schema.id_field)
        {
            let now = match &draft {
                Value::Object(m) => m.get(id_field),
                _ => None,
            };
            if now != stored.get(id_field) && now.is_some() {
                out.push(Violation {
                    field: id_field.clone(),
                    message: "is immutable".to_string(),
                });
            }
        }
        out
    }

    /// Whether Review changes can run.
    pub fn can_review(&self) -> bool {
        if self.busy || !self.problems().is_empty() {
            return false;
        }
        match self.mode {
            EditorMode::Edit => !self.changes().is_empty(),
            EditorMode::Insert => true,
        }
    }

    /// The id to insert under: the typed one, else none.
    pub fn insert_id(&self) -> Option<&str> {
        let id = self.id.trim();
        (!id.is_empty()).then_some(id)
    }

    /// Rebase on a record read after a conflict: the stored side moves, the
    /// edit stays.
    pub fn rebase(&mut self, stored: Record) {
        self.stored = Some(stored);
        self.error = None;
        self.stage = Stage::Editing;
        self.busy = false;
    }
}

/// What a delete removes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Record {
        collection: String,
        id: String,
        revision: u64,
    },
    Edge {
        graph: String,
        id: String,
    },
}

impl DeleteTarget {
    /// The thing, as the dialog names it: `users/u44182` or `edge e_7f3a2d59`.
    pub fn label(&self) -> String {
        match self {
            DeleteTarget::Record { collection, id, .. } => format!("{collection}/{id}"),
            DeleteTarget::Edge { id, .. } => format!("edge {id}"),
        }
    }
}

/// The delete confirmation. The destructive button stays disabled until the
/// word is typed: surrounding whitespace is forgiven, case is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteRequest {
    pub target: DeleteTarget,
    pub typed: String,
    pub busy: bool,
    pub error: Option<EditorError>,
}

impl DeleteRequest {
    pub fn new(target: DeleteTarget) -> Self {
        Self {
            target,
            typed: String::new(),
            busy: false,
            error: None,
        }
    }

    pub fn confirmed(&self) -> bool {
        self.typed.trim() == "delete"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::{Overlay, collection_schema, stored_record};

    fn edit(collection: &str, profile: Profile, id: &str) -> RecordEditor {
        let schema = collection_schema(collection).expect("schema");
        let stored = stored_record(collection, id, &Overlay::default()).expect("record");
        RecordEditor::edit(collection, profile, schema, stored)
    }

    #[test]
    fn an_untouched_edit_has_no_changes_and_cannot_be_reviewed() {
        let e = edit("users", Profile::Document, "u44182");
        assert!(e.changes().is_empty());
        assert!(!e.can_review());
        assert_eq!(e.title(), "Edit record users/u44182, revision 18");
    }

    #[test]
    fn changing_a_field_lists_one_change_and_enables_review() {
        let mut e = edit("users", Profile::Document, "u44182");
        e.text = e
            .text
            .replace("\"status\": \"active\"", "\"status\": \"pending\"");
        let c = e.changes();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].field, "status");
        assert!(e.can_review());
    }

    #[test]
    fn broken_json_blocks_review_and_names_the_error() {
        let mut e = edit("users", Profile::Document, "u44182");
        e.text.push('{');
        assert!(!e.can_review());
        assert!(e.problems()[0].message.starts_with("Invalid JSON"));
    }

    #[test]
    fn the_id_cannot_change_on_edit() {
        let mut e = edit("users", Profile::Document, "u44182");
        e.text = e.text.replace("\"id\": \"u44182\"", "\"id\": \"u1\"");
        assert!(
            e.problems()
                .iter()
                .any(|p| p.field == "id" && p.message == "is immutable")
        );
        assert!(!e.can_review());
    }

    #[test]
    fn a_strict_collection_blocks_a_missing_required_field() {
        let mut e = edit("orders", Profile::DocumentStrict, "o104");
        e.text = e.text.replace("\"customer_id\": \"c203\",\n", "");
        assert!(e.problems().iter().any(|p| p.field == "customer_id"));
    }

    #[test]
    fn vectors_stay_out_of_the_text_and_come_back_in_the_draft() {
        let e = edit("articles", Profile::Document, "art104");
        assert!(!e.text.contains("embedding"));
        let draft = e.draft().expect("draft");
        match draft {
            Value::Object(m) => assert!(m.contains_key("embedding")),
            other => panic!("not an object: {other:?}"),
        }
        assert!(e.changes().is_empty());
    }

    #[test]
    fn an_insert_template_holds_the_required_fields_and_everything_is_added() {
        let schema = collection_schema("orders").expect("schema");
        let e = RecordEditor::insert("orders", Profile::DocumentStrict, schema);
        assert!(e.text.contains("\"order_id\""));
        assert!(!e.text.contains("\"items\""));
        assert!(e.insert_id().is_none());
        assert!(e.changes().iter().all(|c| c.before.is_none()));
    }

    #[test]
    fn a_conflict_keeps_the_edit_and_moves_the_stored_side() {
        let mut e = edit("users", Profile::Document, "u44182");
        e.text = e.text.replace("\"active\"", "\"pending\"");
        e.stage = Stage::Reviewing;
        e.busy = true;
        e.error = Some(EditorError::Conflict {
            expected: 18,
            stored: 19,
        });
        let mut newer = e.stored.clone().expect("stored");
        newer.revision = 19;
        e.rebase(newer);
        assert_eq!(e.stored.as_ref().map(|r| r.revision), Some(19));
        assert!(e.text.contains("pending"));
        assert_eq!(
            (e.stage, e.busy, e.error.clone()),
            (Stage::Editing, false, None)
        );
    }

    #[test]
    fn delete_needs_the_exact_word() {
        let mut d = DeleteRequest::new(DeleteTarget::Edge {
            graph: "g".into(),
            id: "e_1".into(),
        });
        assert!(!d.confirmed());
        d.typed = " delete\n".into();
        assert!(d.confirmed());
        d.typed = "Delete".into();
        assert!(!d.confirmed());
        assert_eq!(d.target.label(), "edge e_1");
    }

    #[test]
    fn errors_map_to_their_own_kind() {
        let conflict = StudioError::RevisionConflict {
            collection: "users".into(),
            id: "u".into(),
            expected: 1,
            stored: 2,
        };
        assert_eq!(
            EditorError::from(&conflict),
            EditorError::Conflict {
                expected: 1,
                stored: 2
            }
        );
        assert_eq!(
            EditorError::from(&StudioError::RecordMissing {
                collection: "c".into(),
                id: "i".into()
            }),
            EditorError::Gone
        );
    }
}
