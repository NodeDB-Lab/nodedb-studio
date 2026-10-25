//! Server config: the cluster defaults and the node overrides, with the
//! review of a proposed value.

use crate::data::mock::admin::store::{AdminStore, missing};
use crate::data::mock::cluster::members::{node_id, node_name};
use crate::models::admin::{AdminReview, ReviewLine};
use crate::models::admin_audit::TargetType;
use crate::models::admin_config::{
    ConfigItem, ConfigProposal, ConfigRequest, ConfigScope, ConfigSetting, ConfigSource,
    ConfigTarget, ConfigView, Validation, difference,
};
use crate::services::error::StudioError;

/// How much memory each node has. A budget above it cannot apply.
const NODE_MEMORY_GIB: i64 = 16;
/// The node that cannot be reached, so its config cannot be read.
const UNREACHABLE: u32 = 5;

fn setting(
    key: &str,
    value: &str,
    restart: bool,
    choices: &[&str],
    range: &str,
    description: &str,
) -> ConfigSetting {
    ConfigSetting {
        key: key.into(),
        current: value.into(),
        cluster_default: value.into(),
        source: ConfigSource::ClusterDefault,
        restart_required: restart,
        choices: choices.iter().map(|c| c.to_string()).collect(),
        range: range.into(),
        description: description.into(),
    }
}

pub(super) fn seeded_settings() -> Vec<ConfigSetting> {
    vec![
        setting(
            "query_timeout",
            "30s",
            false,
            &["10s", "30s", "60s", "120s", "300s", "600s"],
            "1s – 300s",
            "Longest a query runs before the server cancels it",
        ),
        setting(
            "memory_budget",
            "8 GiB",
            true,
            &["4 GiB", "8 GiB", "16 GiB", "32 GiB"],
            "1 GiB – 16 GiB",
            "Memory the data plane may use",
        ),
        setting(
            "max_connections",
            "1000",
            false,
            &["500", "1000", "2000", "5000", "20000"],
            "1 – 10000",
            "Client connections a node accepts",
        ),
        setting(
            "enable_vector",
            "true",
            false,
            &["true", "false"],
            "true or false",
            "Serve vector indexes",
        ),
        setting(
            "compaction_interval",
            "6h",
            true,
            &["1h", "3h", "6h", "12h", "24h"],
            "1h – 24h",
            "Time between storage compactions",
        ),
        setting(
            "log_level",
            "info",
            false,
            &["error", "warn", "info", "debug", "trace"],
            "error, warn, info, debug, or trace",
            "Detail the server logs",
        ),
    ]
}

pub(super) fn seeded_overrides() -> Vec<(String, String, String)> {
    vec![(node_id(2), "query_timeout".into(), "30s".into())]
}

/// Whether the server accepts `value` for `key`.
fn validate(key: &str, value: &str) -> Validation {
    let fail = |why: &str| Validation::Failed(why.to_string());
    let number = |text: &str, unit: &str| -> Option<i64> {
        text.trim()
            .strip_suffix(unit)
            .and_then(|n| n.trim().parse::<i64>().ok())
    };
    match key {
        "query_timeout" => {
            let secs = number(value, "s").or_else(|| number(value, "m").map(|m| m * 60));
            match secs {
                Some(s) if (1..=300).contains(&s) => Validation::Passed,
                Some(_) => fail("Use a value from 1s to 300s."),
                None => fail("Enter seconds, such as 60s."),
            }
        }
        "memory_budget" => match number(value, "GiB") {
            Some(g) if (1..=NODE_MEMORY_GIB).contains(&g) => Validation::Passed,
            Some(_) => fail("The budget exceeds the 16 GiB each node has."),
            None => fail("Enter GiB, such as 8 GiB."),
        },
        "max_connections" => match value.trim().parse::<i64>() {
            Ok(n) if (1..=10_000).contains(&n) => Validation::Passed,
            Ok(_) => fail("Use a value from 1 to 10000."),
            Err(_) => fail("Enter a whole number."),
        },
        "enable_vector" => match value {
            "true" | "false" => Validation::Passed,
            _ => fail("Use true or false."),
        },
        "compaction_interval" => match number(value, "h") {
            Some(h) if (1..=24).contains(&h) => Validation::Passed,
            _ => fail("Use hours from 1h to 24h."),
        },
        "log_level" => match value {
            "error" | "warn" | "info" | "debug" | "trace" => Validation::Passed,
            _ => fail("Use error, warn, info, debug, or trace."),
        },
        _ => fail("The server has no such setting."),
    }
}

impl AdminStore {
    /// The member id for a node target, and its name. Refuses an unknown or
    /// unreachable node: its config cannot be read, and Studio shows no
    /// stale value for it.
    fn node_of(&self, target: &ConfigTarget) -> Result<Option<(String, String)>, StudioError> {
        match (target.scope, target.node.as_deref()) {
            (ConfigScope::Cluster, _) => Ok(None),
            (ConfigScope::Node, None) => Err(missing("node", "(none chosen)")),
            (ConfigScope::Node, Some(id)) => {
                let n = (1..=5u32)
                    .find(|n| node_id(*n) == id)
                    .ok_or_else(|| missing("node", id))?;
                if n == UNREACHABLE {
                    return Err(StudioError::from(
                        nodedb_client::NodeDbError::node_unreachable(node_name(n)),
                    ));
                }
                Ok(Some((id.to_string(), node_name(n))))
            }
        }
    }

    fn value_at(&self, node: Option<&str>, s: &ConfigSetting) -> (String, ConfigSource) {
        let over = node.and_then(|n| {
            self.overrides
                .iter()
                .find(|(o, k, _)| o == n && *k == s.key)
                .map(|(_, _, v)| v.clone())
        });
        match over {
            Some(v) => (v, ConfigSource::Runtime),
            None => (s.cluster_default.clone(), ConfigSource::ClusterDefault),
        }
    }

    pub fn config_view(&self, target: &ConfigTarget) -> Result<ConfigView, StudioError> {
        let node = self.node_of(target)?;
        let settings = self
            .cluster_config
            .iter()
            .map(|s| {
                let (current, source) = self.value_at(node.as_ref().map(|n| n.0.as_str()), s);
                ConfigSetting {
                    current,
                    source,
                    ..s.clone()
                }
            })
            .collect();
        Ok(ConfigView {
            target_name: node
                .map(|n| n.1)
                .unwrap_or_else(|| "the cluster".to_string()),
            settings,
            observed_at: self.clock,
        })
    }

    pub fn config_proposal(&self, req: &ConfigRequest) -> Result<ConfigProposal, StudioError> {
        let node = self.node_of(&req.target)?;
        let node_id = node.as_ref().map(|n| n.0.as_str());
        let mut items = Vec::new();
        for (key, value) in &req.changes {
            let s = self
                .cluster_config
                .iter()
                .find(|s| s.key == *key)
                .ok_or_else(|| missing("config setting", key))?;
            let (current, _) = self.value_at(node_id, s);
            let source_after = match (req.target.scope, *value == s.cluster_default) {
                (ConfigScope::Cluster, _) => "Cluster default".to_string(),
                (ConfigScope::Node, true) => "Cluster default".to_string(),
                (ConfigScope::Node, false) => "Runtime (node override)".to_string(),
            };
            items.push(ConfigItem {
                key: key.clone(),
                current: current.clone(),
                effective: value.clone(),
                inherited: s.cluster_default.clone(),
                difference: difference(&current, value),
                source_after,
                restart_required: s.restart_required,
                validation: validate(key, value),
                range: s.range.clone(),
            });
        }
        let required = match req.target.scope {
            ConfigScope::Cluster => "config.cluster.write",
            ConfigScope::Node => "config.node.write",
        };
        Ok(ConfigProposal {
            target_name: node
                .map(|n| n.1)
                .unwrap_or_else(|| "the cluster".to_string()),
            items,
            permission: self.permission(required),
            revision: self.rev.config,
        })
    }

    pub(super) fn preview_config_review(
        &self,
        req: &ConfigRequest,
    ) -> Result<AdminReview, StudioError> {
        let p = self.config_proposal(req)?;
        let mut review = AdminReview {
            title: "Review config changes".to_string(),
            target: p.target_name.clone(),
            lines: p
                .items
                .iter()
                .map(|i| ReviewLine::new(i.key.clone(), Some(&i.current), i.effective.clone()))
                .collect(),
            notes: vec!["Applying records each change in the audit trail.".to_string()],
            warnings: Vec::new(),
            errors: p
                .items
                .iter()
                .filter_map(|i| match &i.validation {
                    Validation::Passed => None,
                    Validation::Failed(why) => Some(format!("{}: {why}", i.key)),
                })
                .collect(),
            permission: p.permission.clone(),
            revision: p.revision,
            apply_label: "Apply reviewed changes".to_string(),
        };
        if p.restarts() {
            review.warnings.push(format!(
                "{} must restart for {} to take effect.",
                p.target_name,
                p.items
                    .iter()
                    .filter(|i| i.restart_required)
                    .map(|i| i.key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(review)
    }

    pub(super) fn apply_config(&mut self, req: &ConfigRequest) -> Result<String, StudioError> {
        let node = self.node_of(&req.target)?;
        let mut audit = Vec::new();
        let mut restart = Vec::new();
        for (key, value) in &req.changes {
            let idx = self
                .cluster_config
                .iter()
                .position(|s| s.key == *key)
                .ok_or_else(|| missing("config setting", key))?;
            let before;
            match &node {
                Some((id, _)) => {
                    let base = self.cluster_config[idx].clone();
                    before = self.value_at(Some(id), &base).0;
                    self.overrides.retain(|(o, k, _)| !(o == id && k == key));
                    // A value equal to the default is no override: the node
                    // inherits again.
                    if *value != base.cluster_default {
                        self.overrides
                            .push((id.clone(), key.clone(), value.clone()));
                    }
                }
                None => {
                    let s = &mut self.cluster_config[idx];
                    before = s.cluster_default.clone();
                    s.cluster_default = value.clone();
                    s.current = value.clone();
                }
            }
            if self.cluster_config[idx].restart_required {
                restart.push(key.clone());
            }
            let name = node
                .as_ref()
                .map(|n| n.1.clone())
                .unwrap_or_else(|| "cluster".to_string());
            audit.push(self.record(
                "Update config",
                TargetType::Config,
                &format!("config:{key}"),
                &name,
                vec![format!("{key}: {before}")],
                vec![format!("{key}: {value}")],
            ));
        }
        self.rev.config += 1;
        let name = node
            .map(|n| n.1)
            .unwrap_or_else(|| "the cluster".to_string());
        let mut summary = format!(
            "Applied {} config {} on {name}. Recorded as {}.",
            req.changes.len(),
            if req.changes.len() == 1 {
                "change"
            } else {
                "changes"
            },
            audit.join(", ")
        );
        if !restart.is_empty() {
            summary.push_str(&format!(" Restart {name} for {}.", restart.join(", ")));
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node2() -> ConfigTarget {
        ConfigTarget {
            scope: ConfigScope::Node,
            node: Some(node_id(2)),
        }
    }

    fn propose(key: &str, value: &str) -> ConfigRequest {
        ConfigRequest {
            target: node2(),
            changes: vec![(key.into(), value.into())],
        }
    }

    #[test]
    fn node_2_runs_query_timeout_at_runtime() {
        let v = AdminStore::seeded().config_view(&node2()).expect("view");
        let q = v
            .settings
            .iter()
            .find(|s| s.key == "query_timeout")
            .expect("q");
        assert_eq!(
            (q.current.as_str(), q.source),
            ("30s", ConfigSource::Runtime)
        );
        let m = v
            .settings
            .iter()
            .find(|s| s.key == "memory_budget")
            .expect("m");
        assert!(m.restart_required);
        assert_eq!(m.source, ConfigSource::ClusterDefault);
    }

    #[test]
    fn an_unreachable_node_shows_no_config() {
        let t = ConfigTarget {
            scope: ConfigScope::Node,
            node: Some(node_id(5)),
        };
        assert!(AdminStore::seeded().config_view(&t).is_err());
    }

    #[test]
    fn sixty_seconds_passes_and_names_the_difference() {
        let p = AdminStore::seeded()
            .config_proposal(&propose("query_timeout", "60s"))
            .expect("proposal");
        let i = &p.items[0];
        assert_eq!(i.difference, "+30s");
        assert_eq!(i.inherited, "30s");
        assert_eq!(i.source_after, "Runtime (node override)");
        assert!(p.can_apply() && !p.restarts());
    }

    #[test]
    fn a_budget_above_the_node_memory_fails_validation() {
        let p = AdminStore::seeded()
            .config_proposal(&propose("memory_budget", "32 GiB"))
            .expect("proposal");
        assert!(!p.can_apply());
        assert!(p.restarts());
    }
}
