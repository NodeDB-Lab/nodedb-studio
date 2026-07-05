//! Streams-tier reads at the backend seam. One method per Streams screen's data.
//! Today: CDC. Notify/MV/Topics/Cron methods are added when those screens are built.

use async_trait::async_trait;

use crate::models::cdc::{CdcOp, CdcRow};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait StreamsData {
    /// The CDC change feed, newest first.
    async fn cdc_feed(&self) -> Result<Vec<CdcRow>, StudioError>;
}

/// Build display rows from the static mock change feed. Lives here (not in the
/// view) so the seam — not the screen — owns the native-doc → display mapping.
pub(crate) fn cdc_rows_from_mock() -> Vec<CdcRow> {
    use crate::data::mock;
    mock::cdc_events()
        .into_iter()
        .enumerate()
        .map(|(i, ev)| {
            let mut payload_json = sonic_rs::to_string(&ev.payload).unwrap_or_default();
            if let Some(note) = ev.note {
                payload_json.push(' ');
                payload_json.push_str(note);
            }
            let op = match ev.op {
                mock::ChangeOp::Insert => CdcOp::Insert,
                mock::ChangeOp::Update => CdcOp::Update,
                mock::ChangeOp::Delete => CdcOp::Delete,
            };
            CdcRow {
                id: format!("cdc-{i}"),
                time: ev.time.to_string(),
                op,
                collection: ev.collection.to_string(),
                payload_json,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_rows_have_unique_stable_ids() {
        let rows = cdc_rows_from_mock();
        assert!(!rows.is_empty());
        let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "ids must be unique");
        assert_eq!(rows[0].id, "cdc-0");
    }
}
