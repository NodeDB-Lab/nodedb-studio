//! What every membership operation shares: the permission check, the impact
//! label, and the result. An operation is previewed first and applied only
//! against the revision the preview read, so a stale preview never applies.

use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;

/// Whether the connected principal may run the operation. The server answers
/// it; a hidden or disabled control never stands in for that answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionCheck {
    /// The privilege the operation needs, e.g. `cluster.membership.write`.
    pub required: String,
    pub granted: bool,
}

/// The effect of a move on running queries. `Unknown` is a real answer: the
/// server cannot predict load, so Studio never invents a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Impact {
    Unknown,
    Low,
    Moderate,
    High,
}

impl Impact {
    pub fn label(self) -> &'static str {
        match self {
            Impact::Unknown => "Unknown",
            Impact::Low => "Low",
            Impact::Moderate => "Moderate",
            Impact::High => "High",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Impact::Unknown | Impact::Moderate => Tone::Warn,
            Impact::Low => Tone::Ok,
            Impact::High => Tone::Err,
        }
    }
}

/// What an applied operation returns: its stable id and what it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpResult {
    /// Stable operation id, e.g. `op_73`. It also tags the activity events.
    pub operation: String,
    pub summary: String,
}

/// Quorum arithmetic for `total` voters.
pub fn quorum_of(total: u32) -> u32 {
    total / 2 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quorum_is_a_majority() {
        assert_eq!(quorum_of(3), 2);
        assert_eq!(quorum_of(4), 3);
        assert_eq!(quorum_of(5), 3);
        assert_eq!(quorum_of(1), 1);
    }

    #[test]
    fn unknown_impact_is_labelled_not_guessed() {
        assert_eq!(Impact::Unknown.label(), "Unknown");
        assert_eq!(Impact::Unknown.tone(), Tone::Warn);
    }
}
