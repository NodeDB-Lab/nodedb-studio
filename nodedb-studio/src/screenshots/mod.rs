//! Screenshot harness: drives the real app through a fixed scene list and
//! writes one PNG per scene and theme. Built only with the `screenshots`
//! feature on Linux. `scripts/screenshots.sh` runs it.

pub mod capture;
pub mod config;
pub mod designer_scenes;
pub mod driver;
pub mod events_scenes;
pub mod scenes;
pub mod spatial_scenes;
pub mod sync;
pub mod timeseries_scenes;
pub mod vector_scenes;
pub mod workbench_scenes;

pub use config::RunConfig;
pub use driver::ScreenshotDriver;
pub use sync::SceneSync;
