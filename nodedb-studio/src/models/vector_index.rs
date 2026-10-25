//! The Index tab: a snapshot of one vector index's counts, builds, and
//! definition, read from the server.
//!
//! A value the server does not report reads `Unavailable`, never zero. A
//! statistic the connection cannot read reads `Unsupported`. A snapshot that
//! only some shards answered is `Incomplete` and never shows as a total.

/// A statistic the server may not report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reported<T> {
    Value(T),
    /// The server does not report it for this index.
    Unavailable,
    /// This connection cannot read it.
    Unsupported,
}

impl<T: Copy> Reported<T> {
    /// A statistic as the server answered. `supported` is false when the
    /// connection cannot read the statistic at all. `None` from a supported
    /// read means the server left it out.
    pub fn of(value: Option<T>, supported: bool) -> Self {
        match (supported, value) {
            (false, _) => Reported::Unsupported,
            (true, None) => Reported::Unavailable,
            (true, Some(v)) => Reported::Value(v),
        }
    }

    /// The value formatted by `show`, or the reason there is none.
    pub fn text(self, show: impl FnOnce(T) -> String) -> String {
        match self {
            Reported::Value(v) => show(v),
            Reported::Unavailable => "Unavailable".to_string(),
            Reported::Unsupported => "Unsupported on this connection".to_string(),
        }
    }
}

/// The index structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexKind {
    Hnsw,
    HnswPq,
    IvfPq,
    Flat,
    /// A type this Studio does not know. Its name stays readable.
    Other(String),
}

impl IndexKind {
    pub fn label(&self) -> String {
        match self {
            IndexKind::Hnsw => "HNSW".to_string(),
            IndexKind::HnswPq => "HNSW + PQ".to_string(),
            IndexKind::IvfPq => "IVF-PQ".to_string(),
            IndexKind::Flat => "Flat".to_string(),
            IndexKind::Other(name) => name.clone(),
        }
    }

    /// The type from the server's name for it. An unknown name stays
    /// readable instead of reading as HNSW.
    pub fn from_name(name: &str) -> Self {
        match name {
            "hnsw" => IndexKind::Hnsw,
            "hnsw_pq" => IndexKind::HnswPq,
            "ivf_pq" => IndexKind::IvfPq,
            "flat" => IndexKind::Flat,
            other => IndexKind::Other(other.to_string()),
        }
    }
}

/// How sealed segments compress vectors. Separate from the stored precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quantization {
    None,
    Sq8,
    Pq,
    Binary,
    RaBitQ,
    Bbq,
    Other(String),
}

impl Quantization {
    /// The mode from the server's name for it.
    pub fn from_name(name: &str) -> Self {
        match name {
            "none" => Quantization::None,
            "sq8" => Quantization::Sq8,
            "pq" => Quantization::Pq,
            "binary" => Quantization::Binary,
            "rabitq" => Quantization::RaBitQ,
            "bbq" => Quantization::Bbq,
            other => Quantization::Other(other.to_string()),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Quantization::None => "None".to_string(),
            Quantization::Sq8 => "SQ8".to_string(),
            Quantization::Pq => "PQ".to_string(),
            Quantization::Binary => "Binary".to_string(),
            Quantization::RaBitQ => "RaBitQ".to_string(),
            Quantization::Bbq => "BBQ".to_string(),
            Quantization::Other(name) => name.clone(),
        }
    }
}

/// How the raw vectors are stored, read from the catalog. It is never
/// inferred from the quantization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPrecision {
    F32,
    F16,
}

impl RawPrecision {
    /// The precision from the catalog's name for it. `None` for a name this
    /// Studio does not know, which then reads as unavailable.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "f32" => Some(RawPrecision::F32),
            "f16" => Some(RawPrecision::F16),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RawPrecision::F32 => "F32",
            RawPrecision::F16 => "F16",
        }
    }
}

/// The index parameters of an HNSW graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HnswParams {
    pub m: u32,
    pub m0: u32,
    pub ef_construction: u32,
}

/// The training state of an IVF-PQ index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IvfState {
    pub trained: bool,
    pub training_threshold: u64,
    pub cells: u32,
    pub nprobe: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexDefinition {
    pub kind: IndexKind,
    pub quantization: Quantization,
    pub raw_precision: Reported<RawPrecision>,
    pub hnsw: Option<HnswParams>,
    pub ivf: Option<IvfState>,
    pub seal_threshold: u64,
}

/// What the builder is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildState {
    Idle,
    /// Builds wait for the builder. No start time is known.
    Queued(u32),
    /// A build runs. Progress is not reported, so none is shown.
    Building,
}

impl BuildState {
    /// The state from the server's counters: builds waiting, and whether
    /// one runs.
    pub fn from_counts(queued: u32, in_progress: bool) -> Self {
        match (in_progress, queued) {
            (true, _) => BuildState::Building,
            (false, 0) => BuildState::Idle,
            (false, n) => BuildState::Queued(n),
        }
    }

    pub fn label(self) -> String {
        match self {
            BuildState::Idle => "No build in progress".to_string(),
            BuildState::Queued(n) => format!("{n} queued, none running"),
            BuildState::Building => "Build in progress".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorCounts {
    pub sealed: u64,
    pub growing: u64,
    pub tombstoned: u64,
}

impl VectorCounts {
    /// Live vectors: sealed and growing, less the tombstoned. Sealed and
    /// growing counts include the tombstoned vectors.
    pub fn live(self) -> u64 {
        (self.sealed + self.growing).saturating_sub(self.tombstoned)
    }

    /// Tombstoned vectors as a share of every stored vector, 0 to 1.
    pub fn tombstone_ratio(self) -> f64 {
        let stored = self.sealed + self.growing;
        if stored == 0 {
            0.0
        } else {
            self.tombstoned as f64 / stored as f64
        }
    }
}

/// Build counters. Completed and failed count since the core opened the
/// index, so they can reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCounters {
    pub queued: u32,
    pub completed: u64,
    pub failed: u64,
}

/// How much of the collection answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    Complete,
    /// Only some shards answered. Counts are partial, not totals.
    Incomplete {
        answered: u32,
        total: u32,
    },
}

impl Coverage {
    /// Complete when every shard answered.
    pub fn of(answered: u32, total: u32) -> Self {
        if answered >= total {
            Coverage::Complete
        } else {
            Coverage::Incomplete { answered, total }
        }
    }
}

/// One read of a vector index.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexStatus {
    pub collection: String,
    pub field: String,
    pub index: String,
    /// The index revision this snapshot describes.
    pub revision: u64,
    /// Seconds since the Unix epoch.
    pub snapshot_at: i64,
    pub coverage: Coverage,
    pub counts: VectorCounts,
    pub sealed_segments: Reported<u32>,
    /// Approximate resident memory, in bytes.
    pub memory_bytes: Reported<u64>,
    /// Approximate disk use, in bytes.
    pub disk_bytes: Reported<u64>,
    pub build: BuildState,
    pub builds: BuildCounters,
    pub definition: IndexDefinition,
}

impl crate::services::async_state::IsEmpty for IndexStatus {
    fn is_empty(&self) -> bool {
        false
    }
}

/// An estimate in binary units with one decimal, `≈ 1.1 GiB`.
pub fn estimate(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    let (value, unit) = if b >= KIB * KIB * KIB {
        (b / (KIB * KIB * KIB), "GiB")
    } else if b >= KIB * KIB {
        (b / (KIB * KIB), "MiB")
    } else if b >= KIB {
        (b / KIB, "KiB")
    } else {
        return format!("≈ {bytes} B");
    };
    format!("≈ {value:.1} {unit}")
}

/// A share as a percentage with two decimals, `0.18%`.
pub fn percent(ratio: f64) -> String {
    format!("{:.2}%", ratio * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_vectors_are_sealed_plus_growing_less_tombstoned() {
        let c = VectorCounts {
            sealed: 1_100_000,
            growing: 10_492,
            tombstoned: 2_000,
        };
        assert_eq!(c.live(), 1_108_492);
    }

    #[test]
    fn the_tombstone_ratio_rounds_to_two_decimals() {
        let c = VectorCounts {
            sealed: 1_100_000,
            growing: 10_492,
            tombstoned: 2_000,
        };
        assert_eq!(percent(c.tombstone_ratio()), "0.18%");
        let empty = VectorCounts {
            sealed: 0,
            growing: 0,
            tombstoned: 0,
        };
        assert_eq!(empty.tombstone_ratio(), 0.0);
    }

    #[test]
    fn estimates_use_binary_units() {
        assert_eq!(estimate(1_181_116_006), "≈ 1.1 GiB");
        assert_eq!(estimate(3_435_973_837), "≈ 3.2 GiB");
        assert_eq!(estimate(5 * 1024 * 1024), "≈ 5.0 MiB");
        assert_eq!(estimate(900), "≈ 900 B");
    }

    #[test]
    fn a_missing_statistic_never_reads_zero() {
        assert_eq!(
            Reported::<u32>::Unavailable.text(|v| v.to_string()),
            "Unavailable"
        );
        assert!(
            Reported::<u32>::Unsupported
                .text(|v| v.to_string())
                .contains("Unsupported")
        );
        assert_eq!(Reported::Value(17u32).text(|v| v.to_string()), "17");
    }

    #[test]
    fn server_names_decode_and_an_unknown_one_stays_readable() {
        assert_eq!(IndexKind::from_name("hnsw"), IndexKind::Hnsw);
        assert_eq!(IndexKind::from_name("ivf_pq"), IndexKind::IvfPq);
        assert_eq!(IndexKind::from_name("hnsw_pq").label(), "HNSW + PQ");
        assert_eq!(IndexKind::from_name("flat"), IndexKind::Flat);
        let other = IndexKind::from_name("diskann");
        assert_eq!(other.label(), "diskann");
        assert_ne!(other, IndexKind::Hnsw);
        assert_eq!(Quantization::from_name("sq8").label(), "SQ8");
        assert_eq!(Quantization::from_name("rabitq"), Quantization::RaBitQ);
        assert_eq!(Quantization::from_name("none").label(), "None");
        assert_eq!(Quantization::from_name("ternary").label(), "ternary");
        assert_eq!(RawPrecision::from_name("f16"), Some(RawPrecision::F16));
        assert_eq!(RawPrecision::from_name("bf16"), None);
    }

    #[test]
    fn a_statistic_is_unsupported_before_it_is_unavailable() {
        assert_eq!(Reported::of(Some(3u32), true), Reported::Value(3));
        assert_eq!(Reported::<u32>::of(None, true), Reported::Unavailable);
        assert_eq!(Reported::of(Some(3u32), false), Reported::Unsupported);
    }

    #[test]
    fn build_state_follows_the_counters() {
        assert_eq!(BuildState::from_counts(0, false), BuildState::Idle);
        assert_eq!(BuildState::from_counts(2, false), BuildState::Queued(2));
        assert_eq!(BuildState::from_counts(2, true), BuildState::Building);
        assert!(BuildState::Queued(2).label().contains("2 queued"));
    }

    #[test]
    fn a_partial_answer_is_incomplete() {
        assert_eq!(Coverage::of(3, 3), Coverage::Complete);
        assert_eq!(
            Coverage::of(2, 3),
            Coverage::Incomplete {
                answered: 2,
                total: 3
            }
        );
    }
}
