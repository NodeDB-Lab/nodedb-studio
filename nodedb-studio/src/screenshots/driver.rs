//! The scene runner: sets each scene's state through the app's own signals and
//! the backend seam, waits for it to render, and writes a PNG per theme. Exits
//! the process when done: 0 when every scene is written, 1 on the first error.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use dioxus::core::provide_root_context;
use dioxus::prelude::*;

use crate::screenshots::capture::{CaptureError, capture};
use crate::screenshots::config::RunConfig;
use crate::screenshots::scenes::{Overlay, STUDIO_CONNECTION, Scene, Screen, scenes};
use crate::screenshots::sync::SceneTarget;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;
use crate::state::explorer::Selected;
use crate::state::preferences::{Preferences, Theme};
use crate::state::ui::{ModalKind, Popover};

/// Time a scene gets to finish its reads, effects, and paint before capture.
const SETTLE: Duration = Duration::from_millis(600);
/// Longest wait for the saved-connection list to load.
const REGISTRY_WAIT: Duration = Duration::from_secs(10);
const REGISTRY_POLL: Duration = Duration::from_millis(50);

/// Every run captures both themes, into `<out>/light` and `<out>/dark`.
const THEMES: [Theme; 2] = [Theme::Light, Theme::Dark];

/// Stops motion and the text caret, so two runs over the same code write the
/// same pixels.
const FREEZE_CSS: &str = "*, *::before, *::after { animation: none !important; \
    transition: none !important; caret-color: transparent !important; }";

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("connection `{STUDIO_CONNECTION}` is not in the saved list after {REGISTRY_WAIT:?}")]
    RegistryTimeout,
    #[error("connect to `{STUDIO_CONNECTION}` failed: {0}")]
    Connect(Box<StudioError>),
    #[error("cannot create {}: {source}", path.display())]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("scene `{scene}` ({theme}): {source}")]
    Capture {
        scene: &'static str,
        theme: &'static str,
        source: CaptureError,
    },
}

/// The app state a scene sets. Every field is the app's own handle.
#[derive(Clone)]
struct Handles {
    service: Rc<dyn Backend>,
    active: Signal<Option<ActiveConnection>>,
    registry: Signal<Vec<SavedConnection>>,
    prefs: Signal<Preferences>,
    modal: Signal<Option<ModalKind>>,
    popover: Signal<Option<Popover>>,
    palette: Signal<bool>,
    target: Signal<Option<SceneTarget>>,
}

/// Renders nothing. Starts the run when a `RunConfig` root context exists.
#[component]
pub fn ScreenshotDriver() -> Element {
    match try_use_context::<RunConfig>() {
        Some(run) => rsx! { Runner { run } },
        None => rsx! {},
    }
}

#[component]
fn Runner(run: RunConfig) -> Element {
    let handles = Handles {
        service: use_context(),
        active: use_context(),
        registry: use_context(),
        prefs: use_context(),
        modal: use_context(),
        popover: use_context(),
        palette: use_context(),
        // A root context, so `SceneSync` inside the router can read it.
        target: use_hook(|| {
            provide_root_context(Signal::new_in_scope(None::<SceneTarget>, ScopeId::ROOT))
        }),
    };
    use_hook(move || {
        spawn(async move {
            let code = match run_all(handles, &run.out_dir).await {
                Ok(written) => {
                    println!(
                        "nodedb-studio: wrote {written} screenshots to {}",
                        run.out_dir.display()
                    );
                    0
                }
                Err(e) => {
                    eprintln!("nodedb-studio: screenshots: {e}");
                    1
                }
            };
            std::process::exit(code);
        });
    });
    rsx! {
        style { "{FREEZE_CSS}" }
    }
}

async fn run_all(h: Handles, out: &Path) -> Result<usize, DriverError> {
    let all = scenes();
    let mut written = 0;
    for theme in THEMES {
        let dir = out.join(theme.attr());
        std::fs::create_dir_all(&dir).map_err(|source| DriverError::CreateDir {
            path: dir.clone(),
            source,
        })?;
        let mut prefs = h.prefs;
        prefs.write().theme = theme;
        for scene in &all {
            apply(&h, scene).await?;
            tokio::time::sleep(SETTLE).await;
            let path = dir.join(format!("{}.png", scene.name));
            capture(&path)
                .await
                .map_err(|source| DriverError::Capture {
                    scene: scene.name,
                    theme: theme.attr(),
                    source,
                })?;
            written += 1;
        }
    }
    Ok(written)
}

/// Close every overlay, move to the scene's screen, then open its overlay.
async fn apply(h: &Handles, scene: &Scene) -> Result<(), DriverError> {
    let (mut modal, mut popover, mut palette) = (h.modal, h.popover, h.palette);
    let (mut active, mut target) = (h.active, h.target);
    modal.set(None);
    popover.set(None);
    palette.set(false);

    match &scene.screen {
        Screen::Disconnected => {
            target.set(None);
            active.set(None);
        }
        Screen::Studio { route, collection } => {
            let connected = active
                .peek()
                .as_ref()
                .is_some_and(|c| c.name == STUDIO_CONNECTION);
            if !connected {
                let session = connect(h).await?;
                active.set(Some(session));
            }
            target.set(Some(SceneTarget {
                route: route.clone(),
                selection: collection.map(|(name, mode)| Selected {
                    name: name.to_string(),
                    mode,
                }),
            }));
        }
    }

    match scene.overlay {
        Overlay::None => {}
        Overlay::Modal(kind) => modal.set(Some(kind)),
        Overlay::Popover(which) => popover.set(Some(which)),
        Overlay::Palette => palette.set(true),
    }
    Ok(())
}

/// Open `STUDIO_CONNECTION` through the seam, as the Connection Manager does.
async fn connect(h: &Handles) -> Result<ActiveConnection, DriverError> {
    let deadline = Instant::now() + REGISTRY_WAIT;
    let creds = loop {
        let found = h
            .registry
            .peek()
            .iter()
            .find(|c| c.name == STUDIO_CONNECTION)
            .map(SavedConnection::credentials);
        if let Some(creds) = found {
            break creds;
        }
        if Instant::now() >= deadline {
            return Err(DriverError::RegistryTimeout);
        }
        tokio::time::sleep(REGISTRY_POLL).await;
    };
    h.service
        .connect(STUDIO_CONNECTION, &creds)
        .await
        .map_err(|e| DriverError::Connect(Box::new(e)))
}
