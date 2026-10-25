//! Graph Explorer: Cypher, Builder, and GraphRAG over the graph bindings.
//! A graph is an overlay on collections, so every node is a record.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::GraphTab;
use crate::views::graph::builder::BuilderTab;
use crate::views::graph::cypher::CypherTab;
use crate::views::graph::rag::RagPage;

#[component]
pub fn GraphPage(tab: GraphTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(GraphTab::ALL, |t| t.label(), |tab| Route::Graph { tab }),
            current: Route::Graph { tab },
        }
        match tab {
            GraphTab::Cypher => rsx! { CypherTab {} },
            GraphTab::Builder => rsx! { BuilderTab {} },
            GraphTab::GraphRag => rsx! { RagPage {} },
        }
    }
}
