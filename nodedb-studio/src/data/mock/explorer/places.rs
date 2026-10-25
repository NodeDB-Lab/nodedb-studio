//! `places` (Columnar, spatial).

use nodedb_types::Value;
use nodedb_types::geometry::Geometry;

use crate::data::mock::explorer::build::{Dataset, s, unit};
use crate::models::records::Record;

const PLACES: usize = 104;

const CATEGORIES: [&str; 6] = ["cafe", "park", "library", "transit", "museum", "landmark"];

fn point(lon: f64, lat: f64) -> Value {
    Value::Geometry(Geometry::Point {
        coordinates: [lon, lat],
    })
}

fn place(i: usize) -> Record {
    let authored: [(&str, &str, &str, f64, f64); 10] = [
        ("place104", "Cafe North Point", "cafe", -73.9857, 40.7484),
        ("place103", "Riverside Park", "park", -73.9871, 40.7479),
        ("place102", "Central Library", "library", -73.9819, 40.7523),
        ("place101", "Metro Station", "transit", -73.9902, 40.7501),
        ("place100", "City Museum", "museum", -73.9769, 40.7516),
        ("place099", "Riverside Cafe", "cafe", -73.9824, 40.7468),
        ("place098", "Brooklyn Bridge", "landmark", -73.9969, 40.7061),
        ("place097", "East River Park", "park", -73.9784, 40.7431),
        ("place096", "Art Gallery", "museum", -73.9832, 40.7510),
        ("place095", "Union Square", "plaza", -73.9905, 40.7359),
    ];
    let (id, name, category, lon, lat) = match authored.get(i) {
        Some((id, name, category, lon, lat)) => {
            (id.to_string(), name.to_string(), *category, *lon, *lat)
        }
        None => {
            let n = PLACES - i;
            let category = CATEGORIES[i % CATEGORIES.len()];
            let mut chars = category.chars();
            let title = chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default();
            (
                format!("place{n:03}"),
                format!("{title} {n}"),
                category,
                -74.02 + unit(i, 21) * 0.12,
                40.69 + unit(i, 22) * 0.11,
            )
        }
    };
    Record::new(
        &id,
        1,
        vec![
            ("feature_id", s(&id)),
            ("name", s(&name)),
            ("category", s(category)),
            ("geometry", point(lon, lat)),
        ],
    )
}

pub const PLACES_SET: Dataset = Dataset {
    total: PLACES,
    row: place,
};

/// The point a record's geometry holds, as longitude and latitude.
pub fn lon_lat(record: &Record) -> Option<(f64, f64)> {
    match record.get("geometry") {
        Some(Value::Geometry(Geometry::Point { coordinates })) => {
            Some((coordinates[0], coordinates[1]))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_place_holds_a_point_and_ids_are_unique() {
        let mut ids = Vec::new();
        for i in 0..PLACES {
            let r = place(i);
            assert!(lon_lat(&r).is_some(), "{}", r.id);
            ids.push(r.id);
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PLACES);
    }

    #[test]
    fn longitude_comes_first() {
        let (lon, lat) = lon_lat(&place(0)).expect("a point");
        assert!(lon < -70.0 && lat > 40.0);
    }
}
