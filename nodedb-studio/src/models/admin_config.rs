//! Server config: the values a cluster or one node runs with.
//!
//! A change is proposed with an explicit scope and target, reviewed, and
//! applied only on confirmation. The review shows the effective value, the
//! cluster default it overrides, whether the node must restart, and whether
//! the server accepts the value. Studio's own preferences are not config:
//! they live in the Preferences dialog.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigScope {
    Cluster,
    #[default]
    Node,
}

impl ConfigScope {
    pub const ALL: [ConfigScope; 2] = [ConfigScope::Cluster, ConfigScope::Node];

    pub fn label(self) -> &'static str {
        match self {
            ConfigScope::Cluster => "Cluster",
            ConfigScope::Node => "Node",
        }
    }
}

/// What a change applies to. `node` is a member id, set for the Node scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigTarget {
    pub scope: ConfigScope,
    pub node: Option<String>,
}

/// Where a value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    ClusterDefault,
    /// Set on the running node. It lasts until a restart reads the file.
    Runtime,
}

impl ConfigSource {
    pub fn label(self) -> &'static str {
        match self {
            ConfigSource::ClusterDefault => "Cluster default",
            ConfigSource::Runtime => "Runtime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSetting {
    pub key: String,
    pub current: String,
    /// The value the cluster gives every node without an override.
    pub cluster_default: String,
    pub source: ConfigSource,
    pub restart_required: bool,
    /// The values the control offers. The server decides which it accepts.
    pub choices: Vec<String>,
    /// The range the server accepts, e.g. `1s – 300s`.
    pub range: String,
    pub description: String,
}

impl ConfigSetting {
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || self.key.to_lowercase().contains(needle)
            || self.description.to_lowercase().contains(needle)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigView {
    /// `node-2`, or `the cluster`.
    pub target_name: String,
    pub settings: Vec<ConfigSetting>,
    pub observed_at: i64,
}

/// Settings with the values the person proposes for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRequest {
    pub target: ConfigTarget,
    pub changes: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Passed,
    Failed(String),
}

impl Validation {
    pub fn passed(&self) -> bool {
        matches!(self, Validation::Passed)
    }
}

/// The review of one proposed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigItem {
    pub key: String,
    pub current: String,
    pub effective: String,
    pub inherited: String,
    pub difference: String,
    /// What the source becomes, e.g. `Runtime (node override)`.
    pub source_after: String,
    pub restart_required: bool,
    pub validation: Validation,
    pub range: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProposal {
    pub target_name: String,
    pub items: Vec<ConfigItem>,
    pub permission: crate::models::cluster_op::PermissionCheck,
    pub revision: u64,
}

impl ConfigProposal {
    pub fn can_apply(&self) -> bool {
        self.permission.granted
            && !self.items.is_empty()
            && self.items.iter().all(|i| i.validation.passed())
    }

    pub fn restarts(&self) -> bool {
        self.items.iter().any(|i| i.restart_required)
    }

    /// The item to show first: the selected setting if it has a proposal.
    pub fn item_for(&self, key: Option<&str>) -> Option<&ConfigItem> {
        key.and_then(|k| self.items.iter().find(|i| i.key == k))
            .or_else(|| self.items.first())
    }
}

/// `+30s`, `-4 GiB`, `+200`. A change between units, or between words, has
/// no arithmetic difference, so it reads as a change.
pub fn difference(current: &str, proposed: &str) -> String {
    match (split_unit(current), split_unit(proposed)) {
        (Some((a, ua)), Some((b, ub))) if ua == ub => {
            let delta = b - a;
            let sign = if delta < 0 { '-' } else { '+' };
            let sep = if ua.is_empty() || ua.len() <= 1 {
                ""
            } else {
                " "
            };
            format!("{sign}{}{sep}{ua}", delta.abs())
        }
        _ => format!("{current} → {proposed}"),
    }
}

/// `30s` → `(30, "s")`, `8 GiB` → `(8, "GiB")`.
fn split_unit(text: &str) -> Option<(i64, &str)> {
    let text = text.trim();
    let end = text
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let number = text[..end].parse::<i64>().ok()?;
    Some((number, text[end..].trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differences_are_arithmetic_within_a_unit() {
        assert_eq!(difference("30s", "60s"), "+30s");
        assert_eq!(difference("8 GiB", "4 GiB"), "-4 GiB");
        assert_eq!(difference("1000", "1200"), "+200");
        assert_eq!(difference("info", "debug"), "info → debug");
        assert_eq!(difference("30s", "2m"), "30s → 2m");
    }
}
