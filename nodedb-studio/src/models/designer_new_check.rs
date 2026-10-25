//! Create collection form validation: the issues that block staging and the
//! identifier rule they use.

use crate::models::collection::{Attached, Profile};
use crate::models::designer::DesignSchema;
use crate::models::designer_new::{Constraint, NewCollection, Storage};

/// What a validation message points at, so the form can show it in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Name,
    Fields,
    Field(u32),
    Constraint(u32),
    Capability(Attached),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub at: Target,
    pub message: String,
}

fn issue(at: Target, message: impl Into<String>) -> Issue {
    Issue {
        at,
        message: message.into(),
    }
}

/// A name the server accepts: lowercase letters, digits, and underscores,
/// not starting with a digit, at most 63 characters.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && name.len() <= 63
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl NewCollection {
    /// What stops this form from being staged. Empty when it can be.
    /// `schema` is the catalog the new collection joins, drafts included.
    pub fn issues(&self, schema: &DesignSchema) -> Vec<Issue> {
        let mut out = Vec::new();
        if self.name.is_empty() {
            out.push(issue(Target::Name, "Name the collection."));
        } else if !is_identifier(&self.name) {
            out.push(issue(
                Target::Name,
                "Use lowercase letters, digits, and underscores. Start with a letter.",
            ));
        } else if schema.name_taken(&self.name) {
            out.push(issue(
                Target::Name,
                format!("`{}` already exists in {}.", self.name, schema.database),
            ));
        }
        if self.fields.is_empty() {
            out.push(issue(Target::Fields, "Add at least one field."));
        }
        let primaries = self.fields.iter().filter(|f| f.primary).count();
        if self.profile() != Profile::Document && primaries != 1 {
            out.push(issue(
                Target::Fields,
                match self.storage {
                    Storage::KeyValue => "A key-value collection needs exactly one key field.",
                    Storage::Document | Storage::Columnar => "Mark exactly one primary field.",
                },
            ));
        }
        for f in &self.fields {
            if !is_identifier(&f.name) {
                out.push(issue(
                    Target::Field(f.id),
                    "Use lowercase letters, digits, and underscores.",
                ));
            } else if self.fields.iter().filter(|o| o.name == f.name).count() > 1 {
                out.push(issue(Target::Field(f.id), "Field names must be unique."));
            }
        }
        for c in &self.constraints {
            self.check_constraint(c, schema, &mut out);
        }
        if self.attached(Attached::FullText) && self.text_field.is_empty() {
            out.push(issue(
                Target::Capability(Attached::FullText),
                "Add a text field to index.",
            ));
        }
        if self.attached(Attached::Vector) && self.vector_field.is_empty() {
            out.push(issue(
                Target::Capability(Attached::Vector),
                "Add a vector field to index.",
            ));
        }
        out
    }

    fn check_constraint(&self, c: &Constraint, schema: &DesignSchema, out: &mut Vec<Issue>) {
        let at = Target::Constraint(c.id());
        let has = |name: &str| self.fields.iter().any(|f| f.name == name);
        match c {
            Constraint::Unique { fields, .. } => {
                if fields.is_empty() {
                    out.push(issue(at, "Pick the fields that must be unique."));
                } else if let Some(missing) = fields.iter().find(|f| !has(f)) {
                    out.push(issue(at, format!("`{missing}` is not a field.")));
                }
            }
            Constraint::ForeignKey {
                field,
                to_collection,
                to_field,
                ..
            } => {
                if !has(field) {
                    out.push(issue(at, "Pick the field that holds the reference."));
                } else if to_collection.is_empty() || to_field.is_empty() {
                    out.push(issue(at, "Pick the collection and field it references."));
                } else if schema
                    .collection(to_collection)
                    .is_none_or(|t| t.field(to_field).is_none())
                {
                    out.push(issue(
                        at,
                        format!("`{to_collection}.{to_field}` does not exist."),
                    ));
                }
            }
            Constraint::Check { expression, .. } => {
                if expression.trim().is_empty() {
                    out.push(issue(at, "Write the condition a record must meet."));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::Collection;
    use crate::models::designer::{DesignCollection, DesignField, ObjectState};
    use crate::models::designer_new::{ConstraintKind, SchemaMode};
    use crate::models::schema::FieldType;

    fn schema() -> DesignSchema {
        DesignSchema {
            database: "analytics".into(),
            collections: vec![DesignCollection {
                collection: Collection {
                    name: "orders".into(),
                    profile: Profile::DocumentStrict,
                    attached: vec![],
                    description: String::new(),
                    rows: None,
                },
                heading: None,
                fields: vec![DesignField::new("id", FieldType::Uuid).primary()],
                indexes: vec![],
                settings: None,
                state: ObjectState::Applied,
            }],
            off_canvas: vec![Collection {
                name: "places".into(),
                profile: Profile::Spatial,
                attached: vec![],
                description: String::new(),
                rows: None,
            }],
            references: vec![],
        }
    }

    fn named(name: &str) -> NewCollection {
        let mut c = NewCollection::blank("analytics");
        c.name = name.into();
        c
    }

    #[test]
    fn identifiers_are_lowercase_snake_case() {
        assert!(is_identifier("shipments_2"));
        assert!(is_identifier("_tmp"));
        assert!(!is_identifier("2fast"));
        assert!(!is_identifier("Shipments"));
        assert!(!is_identifier("a-b"));
        assert!(!is_identifier(""));
    }

    #[test]
    fn a_blank_form_needs_a_name() {
        let issues = NewCollection::blank("analytics").issues(&schema());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].at, Target::Name);
    }

    #[test]
    fn a_taken_name_is_refused() {
        let issues = named("orders").issues(&schema());
        assert!(issues[0].message.contains("already exists"));
    }

    #[test]
    fn a_collection_off_the_canvas_still_holds_its_name() {
        let issues = named("places").issues(&schema());
        assert!(issues[0].message.contains("already exists"));
    }

    #[test]
    fn a_strict_collection_needs_exactly_one_primary_field() {
        let mut f = named("a");
        f.schema = SchemaMode::Strict;
        assert!(f.issues(&schema()).is_empty());
        f.fields[0].primary = false;
        assert!(
            f.issues(&schema())
                .iter()
                .any(|i| i.at == Target::Fields && i.message.contains("primary"))
        );
    }

    #[test]
    fn a_foreign_key_must_name_an_existing_target() {
        let mut f = named("shipments");
        f.add_field("order_id", FieldType::Uuid, true, false);
        f.add_constraint(ConstraintKind::ForeignKey);
        if let Some(Constraint::ForeignKey {
            field,
            to_collection,
            to_field,
            ..
        }) = f.constraints.first_mut()
        {
            *field = "order_id".into();
            *to_collection = "orders".into();
            *to_field = "missing".into();
        }
        assert!(f.issues(&schema())[0].message.contains("orders.missing"));
        if let Some(Constraint::ForeignKey { to_field, .. }) = f.constraints.first_mut() {
            *to_field = "id".into();
        }
        assert!(f.issues(&schema()).is_empty());
    }

    #[test]
    fn a_capability_without_a_field_is_refused() {
        let mut f = named("a");
        f.set_attached(Attached::Vector, true);
        assert!(
            f.issues(&schema())
                .iter()
                .any(|i| i.at == Target::Capability(Attached::Vector))
        );
    }
}
