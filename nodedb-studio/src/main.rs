//! NodeDB-Studio — desktop GUI client for NodeDB.
//!
//! Entry point: configures the desktop window and launches the root `App`.

mod app;
mod components;
mod data;
mod models;
mod routes;
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

    LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}
