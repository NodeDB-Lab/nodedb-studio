//! Collections as the catalog lists them.
//!
//! NodeDB has no engine per feature. A collection has one storage
//! *profile* (how its records are stored) and any number of *attached
//! capabilities* (Graph, Full-text, Vector) that work over those records.
//! A capability is never a profile: a vector index belongs to a field of a
//! document, and a graph is a set of edges between records.

use serde::{Deserialize, Serialize};

/// How a collection stores its records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Profile {
    /// Schemaless documents.
    Document,
    /// Documents with an enforced schema.
    DocumentStrict,
    KeyValue,
    /// Plain columnar rows.
    Columnar,
    /// Columnar rows ordered by an event-time field.
    Timeseries,
    /// Columnar rows with a geometry field.
    Spatial,
    /// An N-dimensional array of cells.
    Array,
}

impl Profile {
    /// The badge text in the catalog and the collection header.
    pub fn badge(self) -> &'static str {
        match self {
            Profile::Document => "Document",
            Profile::DocumentStrict => "Strict",
            Profile::KeyValue => "KV",
            Profile::Columnar => "Columnar",
            Profile::Timeseries => "Timeseries",
            Profile::Spatial => "Spatial",
            Profile::Array => "Array",
        }
    }

    /// The badge tone class, from `controls.css`.
    pub fn tone(self) -> &'static str {
        match self {
            Profile::Document => "blue",
            Profile::DocumentStrict => "gray",
            Profile::KeyValue => "amber",
            Profile::Columnar => "gray",
            Profile::Timeseries => "green",
            Profile::Spatial => "violet",
            Profile::Array => "blue",
        }
    }

    /// The storage type as the inspector states it.
    pub fn storage_label(self) -> &'static str {
        match self {
            Profile::Document => "Document (flexible)",
            Profile::DocumentStrict => "Document (strict)",
            Profile::KeyValue => "Key-Value",
            Profile::Columnar => "Columnar",
            Profile::Timeseries => "Columnar / Timeseries",
            Profile::Spatial => "Columnar / Spatial",
            Profile::Array => "Array",
        }
    }

    /// The badges of the collection header: the family, then the profile
    /// when it refines the family. `orders` reads `Document` `Strict`.
    pub fn header_badges(self) -> &'static [&'static str] {
        match self {
            Profile::Document => &["Document"],
            Profile::DocumentStrict => &["Document", "Strict"],
            Profile::KeyValue => &["KV"],
            Profile::Columnar => &["Columnar"],
            Profile::Timeseries => &["Columnar", "Timeseries"],
            Profile::Spatial => &["Columnar", "Spatial"],
            Profile::Array => &["Array"],
        }
    }

    /// The one-letter square before a collection name on the Designer canvas.
    pub fn letter(self) -> &'static str {
        match self {
            Profile::Document => "D",
            Profile::DocumentStrict => "S",
            Profile::KeyValue => "K",
            Profile::Columnar => "C",
            Profile::Timeseries => "T",
            Profile::Spatial => "P",
            Profile::Array => "A",
        }
    }

    /// What the records are called in a count badge: `1,284 docs`.
    pub fn short_noun(self) -> &'static str {
        match self {
            Profile::Document | Profile::DocumentStrict => "docs",
            Profile::KeyValue => "keys",
            Profile::Columnar | Profile::Timeseries => "rows",
            Profile::Spatial => "features",
            Profile::Array => "cells",
        }
    }

    /// What the records are called in a page footer: `of 104 documents`.
    pub fn record_noun(self) -> &'static str {
        match self {
            Profile::Document | Profile::DocumentStrict => "documents",
            Profile::KeyValue => "keys",
            Profile::Columnar | Profile::Timeseries => "rows",
            Profile::Spatial => "features",
            Profile::Array => "cells",
        }
    }

    /// The catalog icon.
    pub fn icon(self) -> crate::components::icon::Icon {
        use crate::components::icon::Icon;
        match self {
            Profile::Document | Profile::DocumentStrict => Icon::FileText,
            Profile::KeyValue => Icon::Layers,
            Profile::Columnar => Icon::Table,
            Profile::Timeseries => Icon::ChartColumn,
            Profile::Spatial => Icon::MapPin,
            Profile::Array => Icon::Box,
        }
    }

    /// The Insert button label.
    pub fn insert_label(self) -> &'static str {
        match self {
            Profile::Spatial => "Feature",
            Profile::Document
            | Profile::DocumentStrict
            | Profile::KeyValue
            | Profile::Columnar
            | Profile::Timeseries
            | Profile::Array => "Insert",
        }
    }

    /// Whether stored records can be replaced or removed.
    pub fn append_only(self) -> bool {
        match self {
            Profile::Timeseries => true,
            Profile::Document
            | Profile::DocumentStrict
            | Profile::KeyValue
            | Profile::Columnar
            | Profile::Spatial
            | Profile::Array => false,
        }
    }

    /// Whether the server rejects a field outside the schema.
    pub fn enforces_schema(self) -> bool {
        match self {
            Profile::Document | Profile::KeyValue => false,
            Profile::DocumentStrict
            | Profile::Columnar
            | Profile::Timeseries
            | Profile::Spatial
            | Profile::Array => true,
        }
    }

    /// The schema line of the editor's validation panel.
    pub fn schema_label(self) -> &'static str {
        match self {
            Profile::Document => "Flexible (schemaless)",
            Profile::KeyValue => "Free-form value",
            Profile::DocumentStrict => "Strict (enforced)",
            Profile::Columnar | Profile::Timeseries | Profile::Spatial | Profile::Array => {
                "Typed columns (enforced)"
            }
        }
    }
}

/// A capability attached to a collection's records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attached {
    Graph,
    FullText,
    Vector,
}

impl Attached {
    pub fn badge(self) -> &'static str {
        match self {
            Attached::Graph => "Graph",
            Attached::FullText => "Text",
            Attached::Vector => "Vector",
        }
    }

    pub fn tone(self) -> &'static str {
        match self {
            Attached::Graph => "violet",
            Attached::FullText => "blue",
            Attached::Vector => "green",
        }
    }
}

/// One catalog entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub profile: Profile,
    pub attached: Vec<Attached>,
    pub description: String,
    /// Records the server reports. `None` when it does not report a count:
    /// never shown as zero.
    pub rows: Option<u64>,
}

impl Collection {
    pub fn has(&self, capability: Attached) -> bool {
        self.attached.contains(&capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_documents_read_as_document_then_strict() {
        assert_eq!(
            Profile::DocumentStrict.header_badges(),
            ["Document", "Strict"]
        );
        assert_eq!(
            Profile::Timeseries.header_badges(),
            ["Columnar", "Timeseries"]
        );
    }

    #[test]
    fn only_timeseries_is_append_only() {
        assert!(Profile::Timeseries.append_only());
        assert!(!Profile::Columnar.append_only());
    }
}
