//! NodeDB-Studio — desktop GUI client for NodeDB.
//!
//! Entry point: configures the desktop window and launches the root `App`.

mod app;
mod components;
mod data;
mod modals;
mod models;
mod routes;
#[cfg(all(feature = "screenshots", target_os = "linux"))]
mod screenshots;
mod services;
mod state;
mod views;

use dioxus::LaunchBuilder;
use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{Config, WindowBuilder};

use crate::app::App;

fn main() {
    let window = WindowBuilder::new()
        .with_title("NodeDB Studio")
        .with_inner_size(LogicalSize::new(1440.0, 900.0));

    let builder = LaunchBuilder::desktop().with_cfg(Config::new().with_window(window));

    // `--screenshots <dir>` runs the scene runner instead of an interactive
    // session.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    let builder = match screenshots::RunConfig::from_args(std::env::args().skip(1)) {
        Ok(Some(run)) => builder.with_context(run),
        Ok(None) => builder,
        Err(e) => {
            eprintln!("nodedb-studio: {e}");
            std::process::exit(2);
        }
    };

    builder.launch(App);
}
