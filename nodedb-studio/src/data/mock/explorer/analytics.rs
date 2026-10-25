//! `sales` (Columnar, timeseries) and `inventory` (Columnar).

use crate::data::mock::explorer::build::{Dataset, at, dec, int, s, ts, unit};
use crate::models::format::epoch;
use crate::models::records::Record;

const SALES: usize = 12_482;

/// Id, day/hour/minute/second of 2026-06, store, product, quantity, revenue.
type Sale = (
    &'static str,
    (u32, u32, u32, u32),
    &'static str,
    &'static str,
    i64,
    &'static str,
);

/// Sku, stock, price, day/hour/minute/second of 2024-06, `_id`.
type Stock = (
    &'static str,
    i64,
    &'static str,
    (u32, u32, u32, u32),
    &'static str,
);

fn sale(i: usize) -> Record {
    let authored: [Sale; 10] = [
        (
            "sal104",
            (13, 13, 21, 14),
            "store_104",
            "pr_8831",
            3,
            "59.97",
        ),
        (
            "sal103",
            (13, 12, 48, 2),
            "store_017",
            "pr_4410",
            1,
            "19.99",
        ),
        (
            "sal102",
            (13, 12, 17, 45),
            "store_205",
            "pr_9920",
            2,
            "49.98",
        ),
        ("sal101", (13, 11, 3, 28), "store_104", "pr_3312", 1, "9.99"),
        (
            "sal100",
            (13, 10, 42, 11),
            "store_078",
            "pr_5567",
            4,
            "79.96",
        ),
        ("sal099", (13, 9, 56, 3), "store_017", "pr_2201", 2, "39.98"),
        (
            "sal098",
            (13, 9, 11, 37),
            "store_301",
            "pr_7788",
            1,
            "14.99",
        ),
        (
            "sal097",
            (13, 8, 33, 50),
            "store_104",
            "pr_6642",
            5,
            "124.95",
        ),
        (
            "sal096",
            (13, 7, 20, 16),
            "store_219",
            "pr_1188",
            1,
            "29.99",
        ),
        (
            "sal095",
            (12, 23, 58, 44),
            "store_017",
            "pr_4421",
            3,
            "89.97",
        ),
    ];
    if let Some((id, (d, h, mi, sec), store, product, qty, revenue)) = authored.get(i) {
        return Record::new(
            id,
            1,
            vec![
                ("sold_at", ts(2026, 6, *d, *h, *mi, *sec)),
                ("store_id", s(store)),
                ("product_id", s(product)),
                ("quantity", int(*qty)),
                ("revenue", dec(revenue)),
                ("currency", s("USD")),
            ],
        );
    }
    // 40 seconds apart, newest first: every row stays inside seven days.
    let sold = epoch(2026, 6, 12, 23, 58, 44) - (i as i64 - 9) * 40;
    let qty = 1 + (unit(i, 3) * 5.0) as i64;
    let cents = 999 * qty;
    Record::new(
        &format!("sal{:06}", 100_000 + SALES - i),
        1,
        vec![
            ("sold_at", at(sold)),
            (
                "store_id",
                s(&format!("store_{:03}", 1 + (unit(i, 4) * 320.0) as u32)),
            ),
            (
                "product_id",
                s(&format!("pr_{}", 1000 + (unit(i, 5) * 9000.0) as u32)),
            ),
            ("quantity", int(qty)),
            (
                "revenue",
                dec(&format!("{}.{:02}", cents / 100, cents % 100)),
            ),
            ("currency", s("USD")),
        ],
    )
}

pub const SALES_SET: Dataset = Dataset {
    total: SALES,
    row: sale,
};

const INVENTORY: usize = 9482;

fn warehouse(i: usize) -> &'static str {
    // 41 is coprime with 9472, so exactly 48 of the 9472 later rows land
    // under 48; with the ten authored rows that is 58 `nyc` rows.
    if i < 10 || ((i - 10) * 41) % 9472 < 48 {
        return "nyc";
    }
    ["sfo", "lon", "sin"][i % 3]
}

fn stock_item(i: usize) -> Record {
    let authored: [Stock; 10] = [
        (
            "skuA104",
            240,
            "12.50",
            (13, 10, 24, 17),
            "684b9f3a7e2c4d00123ab456",
        ),
        (
            "skuA217",
            18,
            "99.00",
            (13, 9, 11, 3),
            "684b9f3a7e2c4d00123ab457",
        ),
        (
            "skuA328",
            0,
            "7.99",
            (12, 18, 45, 21),
            "684b9f3a7e2c4d00123ab458",
        ),
        (
            "skuA406",
            320,
            "25.00",
            (12, 16, 3, 12),
            "684b9f3a7e2c4d00123ab459",
        ),
        (
            "skuB011",
            75,
            "49.90",
            (12, 14, 27, 8),
            "684b9f3a7e2c4d00123ab45a",
        ),
        (
            "skuB093",
            12,
            "15.75",
            (12, 11, 16, 42),
            "684b9f3a7e2c4d00123ab45b",
        ),
        (
            "skuB228",
            210,
            "8.40",
            (12, 9, 58, 31),
            "684b9f3a7e2c4d00123ab45c",
        ),
        (
            "skuC001",
            5,
            "120.00",
            (11, 19, 4, 55),
            "684b9f3a7e2c4d00123ab45d",
        ),
        (
            "skuC120",
            60,
            "32.10",
            (11, 15, 20, 16),
            "684b9f3a7e2c4d00123ab45e",
        ),
        (
            "skuC340",
            0,
            "6.25",
            (11, 12, 41, 9),
            "684b9f3a7e2c4d00123ab45f",
        ),
    ];
    if let Some((sku, stock, price, (d, h, mi, sec), id)) = authored.get(i) {
        let rev = if i == 0 { 7 } else { 2 };
        return Record::new(
            id,
            rev,
            vec![
                ("sku", s(sku)),
                ("warehouse", s("nyc")),
                ("stock", int(*stock)),
                ("price", dec(price)),
                ("updated_at", ts(2024, 6, *d, *h, *mi, *sec)),
            ],
        );
    }
    let letter = ["D", "E", "F", "G"][i % 4];
    let stock = (unit(i, 7) * 400.0) as i64;
    let cents = 500 + (unit(i, 8) * 9500.0) as i64;
    Record::new(
        &format!("684b9f3a7e2c4d00{:08x}", 0x1000_0000 + i),
        1 + (i % 4) as u64,
        vec![
            ("sku", s(&format!("sku{letter}{:03}", i % 1000))),
            ("warehouse", s(warehouse(i))),
            ("stock", int(stock)),
            ("price", dec(&format!("{}.{:02}", cents / 100, cents % 100))),
            (
                "updated_at",
                at(epoch(2024, 6, 11, 12, 0, 0) - (i as i64) * 600),
            ),
        ],
    )
}

pub const INVENTORY_SET: Dataset = Dataset {
    total: INVENTORY,
    row: stock_item,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_eight_inventory_rows_sit_in_nyc() {
        let nyc = (0..INVENTORY_SET.total)
            .filter(|i| (INVENTORY_SET.row)(*i).get("warehouse") == Some(&s("nyc")))
            .count();
        assert_eq!(nyc, 58);
    }

    #[test]
    fn row_ids_are_unique() {
        for set in [SALES_SET, INVENTORY_SET] {
            let mut ids: Vec<String> = (0..set.total).map(|i| (set.row)(i).id).collect();
            let n = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(n, ids.len());
        }
    }

    #[test]
    fn every_sale_falls_inside_seven_days_of_now() {
        use crate::data::mock::explorer::build::NOW;
        use nodedb_types::Value;
        let oldest = (SALES_SET.row)(SALES - 1);
        match oldest.get("sold_at") {
            Some(Value::DateTime(t)) => assert!(NOW - t.unix_secs() < 7 * 86_400),
            other => panic!("not a time: {other:?}"),
        }
    }
}
