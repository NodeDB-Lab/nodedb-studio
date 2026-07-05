use super::docs::{FieldValue, MockDoc, doc};
use FieldValue::{Float, Int, Nested, Str};

/// The change operation in a CDC event.
#[derive(Clone, Copy)]
pub enum ChangeOp {
    Insert,
    Update,
    Delete,
}

/// One row in the Streams · CDC live tail.
pub struct ChangeEvent {
    pub time: &'static str,
    pub op: ChangeOp,
    pub collection: &'static str,
    pub payload: MockDoc,
    /// Optional annotation appended after the document (e.g. `⤳ +1 field`).
    pub note: Option<&'static str>,
}

/// The CDC change feed, newest first.
pub fn cdc_events() -> Vec<ChangeEvent> {
    vec![
        ChangeEvent {
            time: "04:23:18.041",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("page_view")),
                ("user_id", Str("u_44182")),
                ("props", Nested(doc(vec![("path", Str("/dashboard"))]))),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.039",
            op: ChangeOp::Update,
            collection: "sessions",
            payload: doc(vec![
                ("_id", Str("s_88209")),
                ("last_seen", Str("2026-06-13T04:23:18Z")),
            ]),
            note: Some("⤳ +1 field"),
        },
        ChangeEvent {
            time: "04:23:18.037",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("click")),
                ("user_id", Str("u_77103")),
                ("props", Nested(doc(vec![("el", Str("#cta-buy"))]))),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.035",
            op: ChangeOp::Insert,
            collection: "orders",
            payload: doc(vec![
                ("id", Int(442004)),
                ("user_id", Str("u_77103")),
                ("total", Float(89.40)),
                ("currency", Str("USD")),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.033",
            op: ChangeOp::Delete,
            collection: "sessions_cache",
            payload: doc(vec![("key", Str("session:u_91002"))]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.031",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("scroll")),
                ("user_id", Str("u_12998")),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.028",
            op: ChangeOp::Update,
            collection: "users",
            payload: doc(vec![
                ("_id", Str("u_44182")),
                ("last_login", Str("2026-06-13T04:23:18Z")),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.025",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("page_view")),
                ("user_id", Str("u_31001")),
                ("props", Nested(doc(vec![("path", Str("/pricing"))]))),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.022",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("form_submit")),
                ("user_id", Str("u_44182")),
                ("props", Nested(doc(vec![("form", Str("feedback"))]))),
            ]),
            note: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serializes_with_fields_in_declared_order() {
        // The orders INSERT row: keys must come out in insertion order, not
        // the alphabetical/HashMap order a `Value::Object` would impose.
        let json = sonic_rs::to_string(&cdc_events()[3].payload).unwrap();
        assert_eq!(
            json,
            r#"{"id":442004,"user_id":"u_77103","total":89.4,"currency":"USD"}"#
        );
    }

    #[test]
    fn nested_document_serializes() {
        let json = sonic_rs::to_string(&cdc_events()[0].payload).unwrap();
        assert_eq!(
            json,
            r#"{"_id":"evt_01HMNJ…","type":"page_view","user_id":"u_44182","props":{"path":"/dashboard"}}"#
        );
    }
}
