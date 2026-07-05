use serde::ser::{Serialize, SerializeMap, Serializer};

// ── Streams payloads ─────────────────────────────────────────────────────────
//
// The CDC and LISTEN/NOTIFY tails show database records. Those records are
// modelled here as native typed documents (`MockDoc`), mirroring how the real
// `ConnectionService` will hand back `nodedb_types::Value` documents from the
// client. The viewers serialize them to JSON via `sonic_rs` purely for display
// — no JSON strings are carried around as data. `MockDoc` preserves field order
// (unlike `Value::Object`'s `HashMap`), so the rendered JSON is deterministic.

/// A scalar (or nested-document) field value inside a [`MockDoc`].
pub enum FieldValue {
    Str(&'static str),
    Int(i64),
    Float(f64),
    Nested(MockDoc),
}

/// An ordered document: `(key, value)` pairs in display order. Stands in for a
/// `nodedb_types::Value::Object` returned by the client.
pub struct MockDoc(pub Vec<(&'static str, FieldValue)>);

impl Serialize for MockDoc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Serialize for FieldValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            FieldValue::Str(s) => serializer.serialize_str(s),
            FieldValue::Int(n) => serializer.serialize_i64(*n),
            FieldValue::Float(f) => serializer.serialize_f64(*f),
            FieldValue::Nested(doc) => doc.serialize(serializer),
        }
    }
}

pub fn doc(fields: Vec<(&'static str, FieldValue)>) -> MockDoc {
    MockDoc(fields)
}
