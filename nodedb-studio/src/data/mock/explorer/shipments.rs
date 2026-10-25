//! `shipments` (Document, strict). No shipment is delayed, which is what
//! the empty filtered result shows.

use crate::data::mock::explorer::build::{Dataset, at, s};
use crate::models::format::epoch;
use crate::models::records::Record;

const SHIPMENTS: usize = 312;

fn shipment(i: usize) -> Record {
    let status = ["in_transit", "delivered", "pending", "delivered"][i % 4];
    let place = ["Rotterdam", "Singapore", "Los Angeles", "Hamburg", "Busan"];
    let id = format!("shp{:05}", 90_000 - i);
    let scheduled = epoch(2026, 6, 13, 6, 0, 0) - (i as i64) * 5_400;
    Record::new(
        &id,
        1 + (i % 3) as u64,
        vec![
            ("_id", s(&id)),
            ("order_id", s(&format!("o{}", 5000 - i))),
            ("status", s(status)),
            ("origin", s(place[i % place.len()])),
            ("destination", s(place[(i + 2) % place.len()])),
            ("scheduled_at", at(scheduled)),
            ("updated_at", at(scheduled + 1_800)),
        ],
    )
}

pub const SHIPMENTS_SET: Dataset = Dataset {
    total: SHIPMENTS,
    row: shipment,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_delayed() {
        assert!((0..SHIPMENTS).all(|i| shipment(i).get("status") != Some(&s("delayed"))));
    }
}
