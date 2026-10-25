//! The command palette's catalog and search.
//!
//! Plain Rust, so the catalog and the filter are testable without a
//! renderer. The palette component builds the catalog from the live
//! connection, runs `filter` on the typed text, and runs the chosen
//! `PaletteAction`.

use crate::components::icon::Icon;
use crate::models::shell::RecentItem;
use crate::routes::{Destination, NAV_GROUPS, Route};
use crate::state::connection::Capabilities;
use crate::state::connections_registry::SavedConnection;
use crate::state::ui::PrefsPane;
use crate::state::workspace::Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Navigation,
    Actions,
    Recent,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Navigation => "Navigation",
            Group::Actions => "Actions",
            Group::Recent => "Recent",
        }
    }
}

/// What running a command does. The component owns the effects.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteAction {
    Navigate(Route),
    NewConnection,
    ManageConnections,
    Disconnect,
    OpenPreferences(PrefsPane),
    CycleTheme,
    /// Show every panel of the current page again.
    RestoreView,
    SwitchConnection(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Stable key for the list.
    pub id: String,
    pub group: Group,
    pub title: String,
    pub description: String,
    /// Extra words the search matches.
    pub keywords: Vec<&'static str>,
    /// A keyboard shortcut shown at the right, e.g. `Cmd D`.
    pub shortcut: Option<&'static str>,
    pub icon: Icon,
    pub action: PaletteAction,
}

/// What the catalog depends on.
pub struct PaletteContext<'a> {
    pub caps: &'a Capabilities,
    /// The active connection's name.
    pub connection: &'a str,
    pub connections: &'a [SavedConnection],
    pub recents: &'a [RecentItem],
    /// The page the user is on.
    pub surface: Surface,
}

fn describe(d: Destination) -> &'static str {
    match d {
        Destination::Explorer => "Browse collections and records",
        Destination::Query => "Write and run SQL",
        Destination::Designer => "Design schemas and indexes",
        Destination::Events => "Change feeds, topics, and scheduled jobs",
        Destination::Graph => "Open the graph exploration workspace",
        Destination::Vector => "Search and project vector collections",
        Destination::Timeseries => "Explore time series and dashboards",
        Destination::Spatial => "Map and analyze spatial data",
        Destination::Search => "Run keyword, semantic, and hybrid search",
        Destination::Cluster => "Nodes, Raft groups, and placement",
        Destination::Sync => "Lite devices and subscriptions",
        Destination::Administration => "Access, audit, backups, and config",
        Destination::Console => "Run commands in a session",
    }
}

fn command(
    id: impl Into<String>,
    group: Group,
    title: impl Into<String>,
    description: impl Into<String>,
    icon: Icon,
    action: PaletteAction,
) -> Command {
    Command {
        id: id.into(),
        group,
        title: title.into(),
        description: description.into(),
        keywords: Vec::new(),
        shortcut: None,
        icon,
        action,
    }
}

impl Command {
    fn words(mut self, keywords: &[&'static str]) -> Self {
        self.keywords = keywords.to_vec();
        self
    }

    fn shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
}

/// Every command the connection offers, in group order.
pub fn commands(ctx: &PaletteContext) -> Vec<Command> {
    let mut out = Vec::new();
    for group in &NAV_GROUPS {
        for d in group.items {
            if d.required_cap().is_some_and(|c| !ctx.caps.has(c)) {
                continue;
            }
            out.push(command(
                format!("nav:{}", d.label()),
                Group::Navigation,
                format!("Open {}", d.label()),
                describe(*d),
                d.icon(),
                PaletteAction::Navigate(d.route()),
            ));
        }
    }

    if ctx.caps.graph {
        out.push(
            command(
                "action:new-graph-query",
                Group::Actions,
                "New graph query",
                "Create a new graph query in the editor",
                Icon::FilePlus,
                PaletteAction::Navigate(Destination::Graph.route()),
            )
            .words(&["cypher", "editor"]),
        );
    }
    out.push(
        command(
            "action:open-saved-query",
            Group::Actions,
            "Open saved query",
            "Search and open a saved graph query",
            Icon::FolderOpen,
            PaletteAction::Navigate(Route::Query {}),
        )
        .words(&["library", "sql"]),
    );
    // On the Query page the hidden panels are the editor's: results, schema.
    let panel_words: &[&'static str] = if ctx.surface == Surface::Query {
        &["view", "restore", "graph", "cypher", "sql", "editor"]
    } else {
        &["view", "restore", "reset"]
    };
    out.push(
        command(
            "action:show-hidden-panels",
            Group::Actions,
            "Show hidden panels",
            "Reopen hidden panels (results, schema, etc.)",
            Icon::Columns2,
            PaletteAction::RestoreView,
        )
        .words(panel_words),
    );
    out.push(command(
        "action:new-connection",
        Group::Actions,
        "New connection",
        "Save a connection to an Origin cluster or a Lite file",
        Icon::Plus,
        PaletteAction::NewConnection,
    ));
    out.push(command(
        "action:manage-connections",
        Group::Actions,
        "Manage connections",
        "Open the saved connections list",
        Icon::Settings,
        PaletteAction::ManageConnections,
    ));
    out.push(
        command(
            "action:preferences",
            Group::Actions,
            "Open preferences",
            "Appearance, editor, and keyboard",
            Icon::Palette,
            PaletteAction::OpenPreferences(PrefsPane::Appearance),
        )
        .shortcut("Cmd ,"),
    );
    out.push(
        command(
            "action:theme",
            Group::Actions,
            "Change theme",
            "Cycle between light, dark, and system",
            Icon::Sun,
            PaletteAction::CycleTheme,
        )
        .words(&["dark", "light", "appearance"]),
    );
    for c in ctx.connections.iter().filter(|c| c.name != ctx.connection) {
        out.push(
            command(
                format!("switch:{}", c.name),
                Group::Actions,
                format!("Switch to {}", c.name),
                format!("Open a session on {}", c.target.summary()),
                Icon::Database,
                PaletteAction::SwitchConnection(c.name.clone()),
            )
            .words(&["connection"]),
        );
    }
    out.push(
        command(
            "action:disconnect",
            Group::Actions,
            "Disconnect",
            format!("Close the session on {}", ctx.connection),
            Icon::Unplug,
            PaletteAction::Disconnect,
        )
        .shortcut("Cmd D"),
    );

    for r in ctx.recents {
        out.push(
            command(
                format!("recent:{}", r.id),
                Group::Recent,
                r.title.clone(),
                "Open recently used query",
                Icon::History,
                PaletteAction::Navigate(Route::Query {}),
            )
            .words(r.kind.keywords()),
        );
    }
    out
}

/// The commands that match `query`: every word of it appears in the title,
/// the description, or a keyword, ignoring case. Group order holds. Inside a
/// group, a title match comes before a description or keyword match.
pub fn filter(all: &[Command], query: &str) -> Vec<Command> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return all.to_vec();
    }
    let in_title = |c: &Command| {
        let title = c.title.to_lowercase();
        terms.iter().all(|t| title.contains(t))
    };
    let in_any = |c: &Command| {
        let hay = format!(
            "{} {} {}",
            c.title.to_lowercase(),
            c.description.to_lowercase(),
            c.keywords.join(" ").to_lowercase()
        );
        terms.iter().all(|t| hay.contains(t))
    };
    let mut hits: Vec<(usize, Command)> = all
        .iter()
        .filter(|c| in_any(c))
        .map(|c| (usize::from(!in_title(c)), c.clone()))
        .collect();
    // Stable: the group order of `all` and the rank keep their meaning.
    let group_rank = |g: Group| match g {
        Group::Navigation => 0,
        Group::Actions => 1,
        Group::Recent => 2,
    };
    hits.sort_by_key(|(rank, c)| (group_rank(c.group), *rank));
    hits.into_iter().map(|(_, c)| c).collect()
}

/// The selection after an arrow key: wraps at both ends, 0 when empty.
pub fn move_selection(len: usize, current: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    (current as isize + delta).rem_euclid(len) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock;
    use crate::routes::tabs::SearchTab;

    fn all_caps() -> Capabilities {
        mock::connections()
            .into_iter()
            .find(|c| c.name == "staging-cluster")
            .and_then(|c| c.profile)
            .map(|p| p.capabilities)
            .expect("staging profile")
    }

    fn catalog(caps: Capabilities, surface: Surface) -> Vec<Command> {
        let saved = mock::connections();
        let recents = mock::recent_items();
        commands(&PaletteContext {
            caps: &caps,
            connection: "staging-cluster",
            connections: &saved,
            recents: &recents,
            surface,
        })
    }

    fn ids(list: &[Command]) -> Vec<&str> {
        list.iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn command_ids_are_unique() {
        let all = catalog(all_caps(), Surface::Query);
        let mut seen = std::collections::HashSet::new();
        for c in &all {
            assert!(seen.insert(&c.id), "duplicate id {}", c.id);
        }
    }

    /// The design's `graph` search, on the Query page.
    #[test]
    fn graph_search_lists_the_design_rows_in_order() {
        let all = catalog(all_caps(), Surface::Query);
        let hits = filter(&all, "graph");
        assert_eq!(
            ids(&hits),
            [
                "nav:Graph Explorer",
                "action:new-graph-query",
                "action:open-saved-query",
                "action:show-hidden-panels",
                "recent:customer-paths",
            ]
        );
        assert_eq!(hits[0].title, "Open Graph Explorer");
    }

    #[test]
    fn an_empty_query_lists_every_command_in_group_order() {
        let all = catalog(all_caps(), Surface::Explorer);
        assert_eq!(filter(&all, "  "), all);
        let groups: Vec<_> = all.iter().map(|c| c.group).collect();
        let mut sorted = groups.clone();
        sorted.sort_by_key(|g| match g {
            Group::Navigation => 0,
            Group::Actions => 1,
            Group::Recent => 2,
        });
        assert_eq!(groups, sorted);
    }

    #[test]
    fn every_word_must_match() {
        let all = catalog(all_caps(), Surface::Explorer);
        assert!(filter(&all, "graph zzz").is_empty());
        assert!(!filter(&all, "OPEN graph").is_empty());
    }

    /// A connection without Graph offers no graph destination or action.
    #[test]
    fn capability_gate_removes_graph_commands() {
        let mut caps = all_caps();
        caps.graph = false;
        let all = catalog(caps, Surface::Query);
        assert!(!ids(&all).contains(&"nav:Graph Explorer"));
        assert!(!ids(&all).contains(&"action:new-graph-query"));
        assert!(ids(&all).contains(&"nav:Explorer"));
    }

    #[test]
    fn switch_commands_skip_the_current_connection() {
        let all = catalog(all_caps(), Surface::Explorer);
        assert!(!ids(&all).contains(&"switch:staging-cluster"));
        assert!(ids(&all).contains(&"switch:Production"));
    }

    #[test]
    fn selection_wraps_at_both_ends() {
        assert_eq!(move_selection(5, 4, 1), 0);
        assert_eq!(move_selection(5, 0, -1), 4);
        assert_eq!(move_selection(5, 2, 1), 3);
        assert_eq!(move_selection(0, 0, 1), 0);
    }

    #[test]
    fn navigation_opens_each_destination_first_page() {
        let all = catalog(all_caps(), Surface::Explorer);
        let search = all
            .iter()
            .find(|c| c.id == "nav:Search")
            .expect("search command");
        assert_eq!(
            search.action,
            PaletteAction::Navigate(Route::Search {
                tab: SearchTab::default()
            })
        );
    }
}
