//! Typed model for a Streams · CDC live-tail row. Replaces the anonymous tuple
//! the view used to carry, so the row has a stable `id` for keyed lists.

/// The change operation in a CDC event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CdcOp {
    Insert,
    Update,
    Delete,
}

impl CdcOp {
    /// Uppercase label shown in the op column.
    pub fn label(self) -> &'static str {
        match self {
            CdcOp::Insert => "INSERT",
            CdcOp::Update => "UPDATE",
            CdcOp::Delete => "DELETE",
        }
    }

    /// CSS modifier class for the op pill.
    pub fn css(self) -> &'static str {
        match self {
            CdcOp::Insert => "ins",
            CdcOp::Update => "upd",
            CdcOp::Delete => "del",
        }
    }
}

/// One row in the Streams · CDC live tail. `payload_json` is already serialized
/// for display (the seam serializes the native document at its boundary).
#[derive(Clone, PartialEq, Debug)]
pub struct CdcRow {
    pub id: String,
    pub time: String,
    pub op: CdcOp,
    pub collection: String,
    pub payload_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_label_and_css_are_stable() {
        assert_eq!(CdcOp::Insert.label(), "INSERT");
        assert_eq!(CdcOp::Insert.css(), "ins");
        assert_eq!(CdcOp::Update.label(), "UPDATE");
        assert_eq!(CdcOp::Delete.css(), "del");
    }

    #[test]
    fn row_carries_stable_id() {
        let row = CdcRow {
            id: "cdc-0".to_string(),
            time: "04:23:18.041".to_string(),
            op: CdcOp::Insert,
            collection: "events".to_string(),
            payload_json: "{}".to_string(),
        };
        assert_eq!(row.id, "cdc-0");
        assert_eq!(row.op.label(), "INSERT");
    }
}
