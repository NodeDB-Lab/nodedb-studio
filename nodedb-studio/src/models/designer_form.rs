//! The Properties forms that stage an index or a logical reference.

use crate::models::designer::{
    DesignIndex, DesignSchema, IndexChoice, LogicalReference, ObjectState, VectorMetric,
    VectorParams,
};
use crate::models::designer_new::is_identifier;
use crate::models::schema::FieldType;

/// An index definition under edit. `replace` names the staged change it
/// updates, or is `None` for a new index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexForm {
    pub replace: Option<String>,
    pub name: String,
    pub collection: String,
    pub field: String,
    pub choice: IndexChoice,
    pub metric: VectorMetric,
}

impl IndexForm {
    /// A new index on the first field of `collection` that a full-text
    /// index fits, or its first field.
    pub fn new(schema: &DesignSchema, collection: Option<&str>) -> Self {
        let owner = collection
            .and_then(|c| schema.collection(c))
            .or_else(|| schema.collections.first());
        let mut form = Self {
            replace: None,
            name: String::new(),
            collection: owner.map(|c| c.name().to_string()).unwrap_or_default(),
            field: String::new(),
            choice: IndexChoice::Secondary,
            metric: VectorMetric::Cosine,
        };
        form.settle(schema);
        form.name = form.suggested_name();
        form
    }

    /// Change the form, then keep it consistent. A name the user did not
    /// type follows the collection and field.
    pub fn edit(&mut self, schema: &DesignSchema, change: impl FnOnce(&mut Self)) {
        let derived = self.name.is_empty() || self.name == self.suggested_name();
        change(self);
        self.settle(schema);
        if derived {
            self.name = self.suggested_name();
        }
    }

    /// The form for a staged index.
    pub fn of(index: &DesignIndex, change_id: &str) -> Self {
        Self {
            replace: Some(change_id.to_string()),
            name: index.name.clone(),
            collection: index.collection.clone(),
            field: index.fields.first().cloned().unwrap_or_default(),
            choice: index.choice().unwrap_or(IndexChoice::Secondary),
            metric: index.vector.map_or(VectorMetric::Cosine, |v| v.metric),
        }
    }

    /// Keep the field one the collection has and the index type fits, and
    /// the name one derived from them until the user names it.
    pub fn settle(&mut self, schema: &DesignSchema) {
        let Some(owner) = schema.collection(&self.collection) else {
            self.field.clear();
            return;
        };
        let fits: Vec<&str> = owner
            .fields_for(self.choice)
            .iter()
            .map(|f| f.name())
            .collect();
        if !fits.contains(&self.field.as_str()) {
            self.field = fits.first().map(|f| f.to_string()).unwrap_or_default();
        }
    }

    /// The name a form with no typed name proposes.
    pub fn suggested_name(&self) -> String {
        if self.field.is_empty() {
            String::new()
        } else {
            format!("idx_{}_{}", self.collection, self.field)
        }
    }

    /// The staged index this form describes. The server still checks it.
    pub fn to_index(&self, schema: &DesignSchema) -> DesignIndex {
        let dims = schema
            .collection(&self.collection)
            .and_then(|c| c.field(&self.field))
            .and_then(|f| match f.ty() {
                FieldType::Vector { dimensions } => Some(dimensions),
                FieldType::String { .. }
                | FieldType::Int64
                | FieldType::Float64
                | FieldType::Decimal { .. }
                | FieldType::Bool
                | FieldType::Timestamp
                | FieldType::Datetime
                | FieldType::Uuid
                | FieldType::Object
                | FieldType::Array
                | FieldType::Json
                | FieldType::Geometry { .. }
                | FieldType::Bytes => None,
            });
        DesignIndex {
            name: self.name.clone(),
            collection: self.collection.clone(),
            kind: self.choice.kind(),
            unique: self.choice.unique(),
            fields: vec![self.field.clone()],
            vector: dims
                .filter(|_| self.choice == IndexChoice::Vector)
                .map(|dimensions| VectorParams {
                    dimensions,
                    metric: self.metric,
                }),
            state: ObjectState::Draft,
        }
    }

    /// What stops this form from staging. `schema` includes the staged
    /// changes other than the one under edit.
    pub fn issues(&self, schema: &DesignSchema) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.is_empty() {
            out.push("Name the index.".to_string());
        } else if !is_identifier(&self.name) {
            out.push("Use lowercase letters, digits, and underscores in the name.".to_string());
        } else if schema.index(&self.name).is_some() {
            out.push(format!(
                "An index named `{}` already exists. Index names are unique in {}.",
                self.name, schema.database
            ));
        }
        match schema.collection(&self.collection) {
            None => out.push("Pick a collection.".to_string()),
            Some(owner) => {
                if owner.fields_for(self.choice).is_empty() {
                    out.push(format!(
                        "{} has no field a {} index fits.",
                        owner.name(),
                        self.choice.label().to_lowercase()
                    ));
                } else if self.field.is_empty() {
                    out.push("Pick a field.".to_string());
                }
            }
        }
        out
    }
}

/// A logical reference under edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceForm {
    pub from_collection: String,
    pub from_field: String,
    pub to_collection: String,
    pub to_field: String,
}

impl ReferenceForm {
    pub fn new(schema: &DesignSchema, from: Option<&str>) -> Self {
        let first = |name: Option<&str>| {
            name.and_then(|n| schema.collection(n))
                .or_else(|| schema.collections.first())
                .map(|c| c.name().to_string())
                .unwrap_or_default()
        };
        let mut form = Self {
            from_collection: first(from),
            from_field: String::new(),
            to_collection: String::new(),
            to_field: String::new(),
        };
        form.to_collection = schema
            .collections
            .iter()
            .map(|c| c.name())
            .find(|n| *n != form.from_collection)
            .map(str::to_string)
            .unwrap_or_default();
        form.settle(schema);
        form
    }

    /// Keep each field one its collection has.
    pub fn settle(&mut self, schema: &DesignSchema) {
        let pick = |collection: &str, field: &mut String| {
            let names: Vec<&str> = schema
                .collection(collection)
                .map(|c| c.fields.iter().map(|f| f.name()).collect())
                .unwrap_or_default();
            if !names.contains(&field.as_str()) {
                *field = names.first().map(|f| f.to_string()).unwrap_or_default();
            }
        };
        pick(&self.from_collection, &mut self.from_field);
        pick(&self.to_collection, &mut self.to_field);
    }

    pub fn to_reference(&self) -> LogicalReference {
        LogicalReference {
            id: String::new(),
            from_collection: self.from_collection.clone(),
            from_field: self.from_field.clone(),
            to_collection: self.to_collection.clone(),
            to_field: self.to_field.clone(),
            state: ObjectState::Draft,
        }
    }

    pub fn issues(&self, schema: &DesignSchema) -> Vec<String> {
        let mut out = Vec::new();
        if self.from_field.is_empty() || self.to_field.is_empty() {
            out.push("Pick a field on both sides.".to_string());
        } else if schema.references.iter().any(|r| {
            r.from_collection == self.from_collection
                && r.from_field == self.from_field
                && r.to_collection == self.to_collection
                && r.to_field == self.to_field
        }) {
            out.push(format!("{} already exists.", self.to_reference().label()));
        } else if self.from_collection == self.to_collection && self.from_field == self.to_field {
            out.push("A field cannot reference itself.".to_string());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::design_schema;

    #[test]
    fn a_new_index_picks_a_field_its_type_fits() {
        let schema = design_schema();
        let mut f = IndexForm::new(&schema, Some("articles"));
        f.choice = IndexChoice::Vector;
        f.settle(&schema);
        assert_eq!(f.field, "embedding");
        f.choice = IndexChoice::FullText;
        f.settle(&schema);
        assert_eq!(f.field, "id", "the first text field");
        assert_eq!(f.suggested_name(), "idx_articles_id");
    }

    #[test]
    fn a_derived_name_follows_the_field_and_a_typed_one_stays() {
        let schema = design_schema();
        let mut f = IndexForm::new(&schema, Some("users"));
        assert_eq!(f.name, "idx_users_id");
        f.edit(&schema, |f| f.field = "country".into());
        assert_eq!(f.name, "idx_users_country");
        f.name = "mine".into();
        f.edit(&schema, |f| f.field = "email".into());
        assert_eq!(f.name, "mine");
    }

    #[test]
    fn a_vector_index_carries_the_fields_dimensions() {
        let schema = design_schema();
        let mut f = IndexForm::new(&schema, Some("articles"));
        f.choice = IndexChoice::Vector;
        f.name = "idx_v".into();
        f.settle(&schema);
        assert_eq!(f.to_index(&schema).vector.map(|v| v.dimensions), Some(768));
    }

    #[test]
    fn a_taken_name_is_an_issue() {
        let schema = design_schema();
        let mut f = IndexForm::new(&schema, Some("users"));
        f.name = "idx_users_email".into();
        assert!(f.issues(&schema)[0].contains("already exists"));
        f.name = "idx_users_country".into();
        assert!(f.issues(&schema).is_empty());
    }

    #[test]
    fn a_collection_without_a_fitting_field_is_an_issue() {
        let schema = design_schema();
        let mut f = IndexForm::new(&schema, Some("orders"));
        f.name = "idx_v".into();
        f.choice = IndexChoice::Vector;
        f.settle(&schema);
        assert!(f.issues(&schema)[0].contains("no field"));
    }

    #[test]
    fn a_new_reference_starts_between_two_collections() {
        let schema = design_schema();
        let f = ReferenceForm::new(&schema, Some("orders"));
        assert_eq!(f.from_collection, "orders");
        assert_ne!(f.to_collection, "orders");
        assert!(!f.from_field.is_empty() && !f.to_field.is_empty());
    }

    #[test]
    fn a_repeated_reference_is_an_issue() {
        let schema = design_schema();
        let r = schema.references[0].clone();
        let f = ReferenceForm {
            from_collection: r.from_collection,
            from_field: r.from_field,
            to_collection: r.to_collection,
            to_field: r.to_field,
        };
        assert!(f.issues(&schema)[0].contains("already exists"));
    }
}
