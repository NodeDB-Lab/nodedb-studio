//! Page tabs as typed route segments.
//!
//! Every multi-page destination (Events & Jobs, Cluster, Administration, …)
//! carries its tab in the URL, so a tab is addressable and a screenshot scene
//! can open it. Each tab set is an enum: an unknown slug fails to parse
//! instead of silently rendering a default page under the wrong name.

use std::fmt;
use std::str::FromStr;

/// A route segment that names no tab of its set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown tab `{0}`")]
pub struct UnknownTab(pub String);

/// Defines a tab enum with its URL slug and visible label. The first variant
/// is the default page of its destination.
macro_rules! tab_set {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $slug:literal, $label:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Display order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn slug(self) -> &'static str {
                match self {
                    $($name::$variant => $slug),+
                }
            }

            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label),+
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::ALL[0]
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.slug())
            }
        }

        impl FromStr for $name {
            type Err = UnknownTab;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($slug => Ok($name::$variant),)+
                    other => Err(UnknownTab(other.to_string())),
                }
            }
        }
    };
}

tab_set! {
    /// Events & Jobs: application data events and scheduled database work.
    EventsTab {
        ChangeFeeds => "change-feeds", "Change feeds";
        MaterializedViews => "materialized-views", "Materialized views";
        Topics => "topics", "Topics";
        Notifications => "notifications", "Notifications";
        ScheduledJobs => "scheduled-jobs", "Scheduled jobs";
    }
}

tab_set! {
    /// Graph Explorer query modes.
    GraphTab {
        Cypher => "cypher", "Cypher";
        Builder => "builder", "Builder";
        GraphRag => "graphrag", "GraphRAG";
    }
}

tab_set! {
    /// Vector Space views.
    VectorTab {
        Search => "search", "Search";
        Projection => "projection", "Projection";
        Index => "index", "Index";
    }
}

tab_set! {
    /// Timeseries views. Backfill is the job review Lifecycle opens: it is
    /// addressable like a tab and lights Lifecycle, but the strip omits it.
    TimeseriesTab {
        Explore => "explore", "Explore";
        Dashboards => "dashboards", "Dashboards";
        Lifecycle => "lifecycle", "Lifecycle";
        Backfill => "backfill", "Backfill";
    }
}

tab_set! {
    /// Spatial views.
    SpatialTab {
        Map => "map", "Map";
        Analyze => "analyze", "Analyze";
        Raster => "raster", "Raster";
    }
}

tab_set! {
    /// Search views.
    SearchTab {
        Results => "results", "Search";
        Analyze => "analyze", "Analyze";
        Indexes => "indexes", "Indexes";
    }
}

tab_set! {
    /// Cluster operations. Scope is the whole cluster, never one database.
    ClusterTab {
        Nodes => "nodes", "Nodes";
        RaftGroups => "raft-groups", "Raft groups";
        Placement => "placement", "Placement";
        Monitoring => "monitoring", "Monitoring";
        Activity => "activity", "Activity";
    }
}

tab_set! {
    /// Lite-to-Origin synchronization.
    SyncTab {
        Devices => "devices", "Devices";
        Subscriptions => "subscriptions", "Subscriptions";
        RejectedWrites => "rejected-writes", "Rejected writes";
        Activity => "activity", "Activity";
    }
}

tab_set! {
    /// Administration: access, audit, recovery, and server config.
    AdminTab {
        Access => "access", "Access";
        RowPolicies => "row-policies", "Row policies";
        Audit => "audit", "Audit";
        Backups => "backups", "Backups";
        Config => "config", "Config";
        Credentials => "credentials", "Credentials";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips<T>(all: &[T])
    where
        T: Copy + PartialEq + fmt::Debug + fmt::Display + FromStr,
    {
        for tab in all {
            let parsed = tab.to_string().parse::<T>().ok();
            assert_eq!(parsed, Some(*tab), "{tab:?} does not round-trip");
        }
    }

    #[test]
    fn every_slug_parses_back_to_its_tab() {
        round_trips(EventsTab::ALL);
        round_trips(GraphTab::ALL);
        round_trips(VectorTab::ALL);
        round_trips(TimeseriesTab::ALL);
        round_trips(SpatialTab::ALL);
        round_trips(SearchTab::ALL);
        round_trips(ClusterTab::ALL);
        round_trips(SyncTab::ALL);
        round_trips(AdminTab::ALL);
    }

    #[test]
    fn unknown_slug_is_an_error() {
        assert_eq!(
            "cdc".parse::<EventsTab>(),
            Err(UnknownTab("cdc".to_string()))
        );
    }

    #[test]
    fn default_is_the_first_tab() {
        assert_eq!(ClusterTab::default(), ClusterTab::Nodes);
        assert_eq!(AdminTab::default(), AdminTab::Access);
    }
}
