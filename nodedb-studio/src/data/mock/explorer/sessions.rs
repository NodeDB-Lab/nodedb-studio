//! `sessions` (Key-Value).

use nodedb_types::Value;

use crate::data::mock::explorer::build::{Dataset, int, s, unit};
use crate::models::records::Record;

const KEYS: usize = 51_003;

fn entry(key: &str, ty: &str, size: i64, ttl: Option<i64>, value: Value) -> Record {
    Record::new(
        key,
        1,
        vec![
            ("key", s(key)),
            ("type", s(ty)),
            ("size", int(size)),
            ("ttl", ttl.map_or(Value::Null, int)),
            ("value", value),
        ],
    )
}

fn json(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn session(i: usize) -> Record {
    match i {
        0 => entry("feature:checkout", "bool", 5, None, Value::Bool(true)),
        1 => entry("flag:graphrag_v2", "bool", 5, None, Value::Bool(true)),
        2 => entry(
            "session:u_44182",
            "json",
            2150,
            Some(14 * 60),
            json(vec![
                ("user", s("u_44182")),
                ("expires_in", int(14)),
                ("since", s("2024-01-15T10:23:54Z")),
            ]),
        ),
        3 => entry("rate:/api/search", "counter", 8, Some(60), int(491)),
        4 => entry(
            "cache:home_dashboard",
            "bytes",
            18_432,
            Some(2 * 3600),
            Value::Bytes(vec![0; 16]),
        ),
        _ => {
            let user = 10_000 + (i * 7919) % 80_000;
            entry(
                &format!("session:u_{user}_{i}"),
                "json",
                900 + (unit(i, 31) * 2000.0) as i64,
                Some(60 * (1 + (unit(i, 32) * 59.0) as i64)),
                json(vec![
                    ("user", s(&format!("u_{user}"))),
                    ("expires_in", int(1 + (unit(i, 32) * 59.0) as i64)),
                ]),
            )
        }
    }
}

pub const SESSIONS_SET: Dataset = Dataset {
    total: KEYS,
    row: session,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<String> = (0..2000).map(|i| session(i).id).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 2000);
    }
}
