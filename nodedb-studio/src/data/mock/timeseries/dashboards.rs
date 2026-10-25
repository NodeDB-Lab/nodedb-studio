//! Saved dashboards: the workspace documents the library lists.

use crate::models::format::epoch;
use crate::models::records::Filter;
use crate::models::timeseries::{
    Aggregate, Bucket, ChartKind, ExploreQuery, GapPolicy, Measure, TimeSpan,
};
use crate::models::ts_dashboard::{Dashboard, DashboardSummary, PanelDef, Refresh};

/// Why a document write did not apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashError {
    Unknown(String),
    Stale {
        name: String,
        expected: u64,
        stored: u64,
    },
    NameTaken(String),
    Invalid(&'static str),
}

#[derive(Debug, Clone)]
pub struct DashStore {
    docs: Vec<Dashboard>,
    next: u32,
    /// The instant a save is stamped with.
    now: i64,
}

fn week() -> TimeSpan {
    TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0))
}

fn usd() -> Filter {
    Filter {
        field: "currency".into(),
        value: "USD".into(),
    }
}

/// `source` is the collection and its time field.
fn panel(
    id: &str,
    title: &str,
    source: (&str, &str),
    measure: Measure,
    group: &str,
    kind: ChartKind,
    height: u32,
) -> PanelDef {
    PanelDef {
        id: id.into(),
        title: title.into(),
        query: ExploreQuery {
            collection: source.0.into(),
            time_field: source.1.into(),
            measure,
            filters: Vec::new(),
            bucket: Bucket::Day1,
            group_by: (!group.is_empty()).then(|| group.to_string()),
            gap: GapPolicy::Missing,
            span: week(),
        },
        kind,
        height,
    }
}

fn doc(
    id: &str,
    name: &str,
    revision: u64,
    saved_at: i64,
    filters: Vec<Filter>,
    panels: Vec<PanelDef>,
) -> Dashboard {
    Dashboard {
        id: Some(id.into()),
        name: name.into(),
        revision,
        saved_at: Some(saved_at),
        range: week(),
        refresh: Refresh::Off,
        filters,
        panels,
    }
}

impl DashStore {
    pub fn seeded(now: i64) -> Self {
        let revenue = Measure::of(Aggregate::Sum, "revenue");
        Self {
            docs: vec![
                doc(
                    "dash-1",
                    "Sales analysis",
                    4,
                    epoch(2026, 6, 13, 10, 42, 0),
                    vec![usd()],
                    vec![
                        panel(
                            "p1",
                            "Daily revenue",
                            ("sales", "sold_at"),
                            revenue,
                            "store_id",
                            ChartKind::Line,
                            310,
                        ),
                        panel(
                            "p2",
                            "Sales count",
                            ("sales", "sold_at"),
                            Measure::count_rows(),
                            "store_id",
                            ChartKind::Bar,
                            270,
                        ),
                    ],
                ),
                doc(
                    "dash-2",
                    "Order volume",
                    2,
                    epoch(2026, 6, 12, 18, 4, 0),
                    Vec::new(),
                    vec![panel(
                        "p1",
                        "Sales per day",
                        ("sales", "sold_at"),
                        Measure::count_rows(),
                        "",
                        ChartKind::Area,
                        320,
                    )],
                ),
                doc(
                    "dash-3",
                    "Delivery trends",
                    1,
                    epoch(2026, 6, 11, 9, 30, 0),
                    Vec::new(),
                    vec![panel(
                        "p1",
                        "Shipments scheduled",
                        ("shipments", "scheduled_at"),
                        Measure::count_rows(),
                        "status",
                        ChartKind::Bar,
                        320,
                    )],
                ),
            ],
            next: 4,
            now,
        }
    }

    pub fn list(&self) -> Vec<DashboardSummary> {
        self.docs
            .iter()
            .filter_map(|d| {
                Some(DashboardSummary {
                    id: d.id.clone()?,
                    name: d.name.clone(),
                    collections: d.collections(),
                    panels: d.panels.len(),
                    revision: d.revision,
                    saved_at: d.saved_at?,
                })
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Result<Dashboard, DashError> {
        self.docs
            .iter()
            .find(|d| d.id.as_deref() == Some(id))
            .cloned()
            .ok_or_else(|| DashError::Unknown(id.to_string()))
    }

    fn name_free(&self, name: &str, except: Option<&str>) -> bool {
        let key = name.trim().to_lowercase();
        !self
            .docs
            .iter()
            .filter(|d| d.id.as_deref() != except)
            .any(|d| d.name.trim().to_lowercase() == key)
    }

    /// Store `doc` over the document it names, if it read the current
    /// revision.
    pub fn save(&mut self, doc: &Dashboard) -> Result<Dashboard, DashError> {
        let id = doc.id.clone().ok_or(DashError::Invalid(
            "The dashboard has no identity yet: save it under a name first.",
        ))?;
        if let Some(why) = Dashboard::name_issue(&doc.name) {
            return Err(DashError::Invalid(why));
        }
        let stored = self.get(&id)?;
        if stored.revision != doc.revision {
            return Err(DashError::Stale {
                name: stored.name,
                expected: doc.revision,
                stored: stored.revision,
            });
        }
        if !self.name_free(&doc.name, Some(&id)) {
            return Err(DashError::NameTaken(doc.name.trim().to_string()));
        }
        let saved = Dashboard {
            name: doc.name.trim().to_string(),
            revision: stored.revision + 1,
            saved_at: Some(self.now),
            ..doc.clone()
        };
        if let Some(slot) = self.docs.iter_mut().find(|d| d.id.as_deref() == Some(&id)) {
            *slot = saved.clone();
        }
        Ok(saved)
    }

    /// Store a copy of `doc` as a new document. It never overwrites one.
    pub fn save_as(&mut self, name: &str, doc: &Dashboard) -> Result<Dashboard, DashError> {
        if let Some(why) = Dashboard::name_issue(name) {
            return Err(DashError::Invalid(why));
        }
        if !self.name_free(name, None) {
            return Err(DashError::NameTaken(name.trim().to_string()));
        }
        let saved = Dashboard {
            id: Some(format!("dash-{}", self.next)),
            name: name.trim().to_string(),
            revision: 1,
            saved_at: Some(self.now),
            ..doc.clone()
        };
        self.next += 1;
        self.docs.push(saved.clone());
        Ok(saved)
    }

    pub fn rename(&mut self, id: &str, revision: u64, name: &str) -> Result<Dashboard, DashError> {
        let mut doc = self.get(id)?;
        doc.revision = revision;
        doc.name = name.to_string();
        self.save(&doc)
    }

    pub fn duplicate(&mut self, id: &str) -> Result<Dashboard, DashError> {
        let doc = self.get(id)?;
        let mut name = format!("{} copy", doc.name);
        let mut n = 2;
        while !self.name_free(&name, None) {
            name = format!("{} copy {n}", doc.name);
            n += 1;
        }
        self.save_as(&name, &doc)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), DashError> {
        let at = self
            .docs
            .iter()
            .position(|d| d.id.as_deref() == Some(id))
            .ok_or_else(|| DashError::Unknown(id.to_string()))?;
        self.docs.remove(at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

    fn store() -> DashStore {
        DashStore::seeded(NOW)
    }

    #[test]
    fn the_library_lists_the_three_documents_the_design_names() {
        let names: Vec<String> = store().list().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["Sales analysis", "Order volume", "Delivery trends"]);
    }

    #[test]
    fn each_document_names_its_collections() {
        let l = store().list();
        assert_eq!(l[0].collections, ["sales"]);
        assert_eq!(l[2].collections, ["shipments"]);
        assert_eq!(l[0].panels, 2);
    }

    #[test]
    fn a_save_bumps_the_revision_and_stamps_the_time() {
        let mut s = store();
        let mut d = s.get("dash-1").expect("doc");
        d.panels[0].title = "Revenue".into();
        let saved = s.save(&d).expect("saved");
        assert_eq!(saved.revision, 5);
        assert_eq!(saved.saved_at, Some(NOW));
        assert_eq!(s.get("dash-1").expect("doc").panels[0].title, "Revenue");
    }

    #[test]
    fn a_save_from_a_stale_revision_is_refused_and_changes_nothing() {
        let mut s = store();
        let stale = s.get("dash-1").expect("doc");
        let mut first = stale.clone();
        first.panels[0].title = "First".into();
        s.save(&first).expect("saved");
        let mut second = stale;
        second.panels[0].title = "Second".into();
        let err = s.save(&second).expect_err("conflict");
        assert!(matches!(
            err,
            DashError::Stale {
                expected: 4,
                stored: 5,
                ..
            }
        ));
        assert_eq!(s.get("dash-1").expect("doc").panels[0].title, "First");
    }

    #[test]
    fn save_as_makes_a_new_document_and_never_overwrites() {
        let mut s = store();
        let d = s.get("dash-1").expect("doc");
        let copy = s.save_as("Sales analysis 2", &d).expect("copy");
        assert_ne!(copy.id, d.id);
        assert_eq!(copy.revision, 1);
        assert_eq!(s.list().len(), 4);
        assert_eq!(
            s.save_as("sales ANALYSIS", &d),
            Err(DashError::NameTaken("sales ANALYSIS".into()))
        );
    }

    #[test]
    fn an_unsaved_document_cannot_be_saved_in_place() {
        let mut s = store();
        let d = Dashboard::untitled(week());
        assert!(matches!(s.save(&d), Err(DashError::Invalid(_))));
    }

    #[test]
    fn rename_checks_the_revision_and_the_name() {
        let mut s = store();
        assert!(s.rename("dash-2", 1, "Orders").is_err(), "stale revision");
        assert_eq!(
            s.rename("dash-2", 2, "Delivery trends"),
            Err(DashError::NameTaken("Delivery trends".into()))
        );
        let r = s.rename("dash-2", 2, "Orders").expect("renamed");
        assert_eq!((r.name.as_str(), r.revision), ("Orders", 3));
    }

    #[test]
    fn duplicates_get_unique_names() {
        let mut s = store();
        let a = s.duplicate("dash-1").expect("copy");
        let b = s.duplicate("dash-1").expect("copy");
        assert_eq!(a.name, "Sales analysis copy");
        assert_eq!(b.name, "Sales analysis copy 2");
    }

    #[test]
    fn delete_removes_only_the_named_document() {
        let mut s = store();
        s.delete("dash-2").expect("deleted");
        assert!(s.get("dash-2").is_err());
        assert_eq!(s.list().len(), 2);
        assert!(s.delete("dash-2").is_err());
    }

    #[test]
    fn document_ids_are_unique() {
        let mut s = store();
        s.duplicate("dash-1").expect("copy");
        let mut ids: Vec<String> = s.list().into_iter().map(|d| d.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }
}
