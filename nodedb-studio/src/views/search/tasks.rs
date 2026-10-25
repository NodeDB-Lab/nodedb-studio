//! Search writes and hand-offs that outlive a click. Each runs under
//! `spawn_forever` and touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::designer::{DesignerState, Target};
use crate::state::workspace::Workspace;
use crate::views::designer::open_index_form;

pub use crate::views::query::tasks::open_in_query;

/// Open an index's definition in the Designer.
pub fn open_index_in_designer(
    mut designer: Signal<DesignerState>,
    nav: dioxus::router::Navigator,
    index: &str,
) {
    designer.write().select(Target::Index(index.to_string()));
    nav.push(Route::Designer {});
}

/// Open the Designer's New index form on `collection`. The form needs the
/// collection's design, so it reads that first.
pub async fn open_new_index(
    mut designer: Signal<DesignerState>,
    mut layout: Signal<Workspace>,
    backend: Rc<dyn Backend>,
    nav: dioxus::router::Navigator,
    collection: String,
) {
    match backend.design_schema().await {
        Ok(schema) => {
            open_index_form(&mut designer, &mut layout, &schema, Some(&collection));
        }
        Err(_) => {
            // The Designer reads the same schema and shows the error there.
            designer.write().select(Target::Collection(collection));
        }
    }
    nav.push(Route::Designer {});
}
