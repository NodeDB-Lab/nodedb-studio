//! Screenshot harness: drives the real app through a fixed scene list and
//! writes one PNG per scene and theme. Built only with the `screenshots`
//! feature on Linux. `scripts/screenshots.sh` runs it.

pub mod admin_scenes;
pub mod area_scenes;
pub mod capture;
pub mod cluster_scenes;
pub mod config;
pub mod designer_scenes;
pub mod driver;
pub mod events_scenes;
pub mod scene;
pub mod scenes;
pub mod search_scenes;
pub mod spatial_scenes;
pub mod sync;
pub mod sync_scenes;
pub mod timeseries_scenes;
pub mod vector_scenes;
pub mod workbench_scenes;

pub use config::RunConfig;
pub use driver::ScreenshotDriver;
pub use sync::SceneSync;
