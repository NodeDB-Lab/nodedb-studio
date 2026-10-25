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
use crate::screenshots::designer_scenes::DesignerSeed;
use crate::screenshots::scenes::{Overlay, STUDIO_CONNECTION, Scene, Screen, scenes};
use crate::screenshots::sync::SceneTarget;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::connection::ActiveConnection;
use crate::state::connection_form::ConnectionForm;
use crate::state::connections_registry::SavedConnection;
use crate::state::preferences::{Preferences, Theme};
use crate::state::query::TxEpoch;
use crate::state::record_editor::RecordEditor;
use crate::state::ui::{ManagerOpen, ManagerSelection, ModalKind, PaletteQuery, Popover};
use crate::state::workspace::Workspace;

/// Time a scene gets to finish its reads, effects, and paint before capture.
const SETTLE: Duration = Duration::from_millis(600);
/// Longest wait for the saved-connection list to load.
const REGISTRY_WAIT: Duration = Duration::from_secs(10);
const REGISTRY_POLL: Duration = Duration::from_millis(50);
/// Time the studio gets to react to a new connection before a scene sets
/// its selection.
const CONNECT_SETTLE: Duration = Duration::from_millis(300);

/// Every run captures both themes, into `<out>/light` and `<out>/dark`.
const THEMES: [Theme; 2] = [Theme::Light, Theme::Dark];

/// Stops motion and the text caret, so two runs over the same code write the
/// same pixels.
const FREEZE_CSS: &str = "*, *::before, *::after { animation: none !important; \
    transition: none !important; caret-color: transparent !important; }";

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("connect to `{connection}` failed: {source}")]
    Connect {
        connection: &'static str,
        source: Box<StudioError>,
    },
    #[error("connection `{0}` is not in the saved list after {REGISTRY_WAIT:?}")]
    UnknownConnection(&'static str),
    #[error("a scene could not stage its Designer changes: {0}")]
    Seed(Box<StudioError>),
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
    editor: Signal<Option<RecordEditor>>,
    popover: Signal<Option<Popover>>,
    palette: Signal<bool>,
    palette_query: PaletteQuery,
    form: Signal<ConnectionForm>,
    manager: Signal<ManagerOpen>,
    selection: Signal<ManagerSelection>,
    workspace: Signal<Workspace>,
    tx_epoch: TxEpoch,
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
        editor: use_context(),
        popover: use_context(),
        palette: use_context(),
        palette_query: use_context(),
        form: use_context(),
        manager: use_context(),
        selection: use_context(),
        workspace: use_context(),
        tx_epoch: use_context(),
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
    let mut editor = h.editor;
    let (mut active, mut target, mut workspace) = (h.active, h.target, h.workspace);
    let (mut form, mut manager, mut selection) = (h.form, h.manager, h.selection);
    let mut palette_query = h.palette_query.0;
    modal.set(None);
    editor.set(None);
    popover.set(None);
    palette.set(false);
    palette_query.set(String::new());
    manager.set(ManagerOpen::default());
    // Each scene starts from the default layout, so one scene's hidden
    // panels never leak into the next image.
    let mut layout = Workspace::default();
    if let Screen::Studio { route, .. } = &scene.screen {
        for id in scene.hidden {
            layout.set_visible(route.surface(), id, false);
        }
        for (id, px) in scene.sizes {
            layout.set_size(route.surface(), id, *px);
        }
    }
    workspace.set(layout);

    match &scene.screen {
        Screen::Disconnected => {
            target.set(None);
            active.set(None);
            // The manager highlights the studio connection, as the design does.
            selection.set(ManagerSelection(Some(STUDIO_CONNECTION.to_string())));
        }
        Screen::Studio { route, explorer } => {
            let connected = active
                .peek()
                .as_ref()
                .is_some_and(|c| c.name == scene.connection);
            if !connected {
                let session = connect(h, scene.connection).await?;
                active.set(Some(session));
                // The studio clears its selection when the connection
                // changes. Let that settle before the scene sets its own.
                tokio::time::sleep(CONNECT_SETTLE).await;
            }
            // One session transaction, shared by Query and Console: every
            // scene starts with none, then opens the one it needs.
            let _ = h.service.rollback_transaction().await;
            if let Some(access) = scene.transaction {
                let _ = h.service.begin_transaction(access).await;
            }
            // The Designer's staged changes live behind the seam: set them
            // before the page reads.
            if let Some(seed) = &scene.designer {
                stage_designer(h, seed).await?;
            }
            let mut epoch = h.tx_epoch.0;
            let next = *epoch.peek() + 1;
            epoch.set(next);
            target.set(Some(SceneTarget {
                route: route.clone(),
                explorer: explorer.as_ref().map(|seed| seed.target()),
                query: scene.query.as_deref().cloned(),
                console: scene.console.as_deref().cloned(),
                designer: scene.designer.as_deref().map(|d| d.state.clone()),
                events: scene.events.as_deref().cloned(),
                vector: scene.vector.as_deref().cloned(),
                timeseries: scene.timeseries.as_deref().cloned(),
                spatial: scene.spatial.as_deref().cloned(),
                search: scene.search.as_deref().cloned(),
                cluster: scene.cluster.as_deref().cloned(),
                sync: scene.sync.as_deref().cloned(),
                admin: scene.admin.as_deref().cloned(),
            }));
        }
    }

    match &scene.overlay {
        Overlay::None => {}
        Overlay::Modal(kind) => modal.set(Some(*kind)),
        Overlay::NewConnection(fields) => {
            form.set((**fields).clone());
            modal.set(Some(ModalKind::NewConnection));
        }
        Overlay::Popover(which) => popover.set(Some(*which)),
        Overlay::RecordEditor(open) => {
            editor.set(Some((**open).clone()));
            modal.set(Some(ModalKind::RecordEditor));
        }
        Overlay::Palette(text) => {
            palette_query.set((*text).to_string());
            palette.set(true);
        }
    }
    Ok(())
}

/// Replace the staged Designer changes with the seed's.
async fn stage_designer(h: &Handles, seed: &DesignerSeed) -> Result<(), DriverError> {
    let fail = |e| DriverError::Seed(Box::new(e));
    h.service.discard_all_changes().await.map_err(fail)?;
    for kind in &seed.pending {
        h.service
            .stage_change(None, kind.clone())
            .await
            .map_err(fail)?;
    }
    Ok(())
}

/// Open the saved connection `name` through the seam, as the Connection
/// Manager does.
async fn connect(h: &Handles, name: &'static str) -> Result<ActiveConnection, DriverError> {
    let deadline = Instant::now() + REGISTRY_WAIT;
    let creds = loop {
        let found = h
            .registry
            .peek()
            .iter()
            .find(|c| c.name == name)
            .map(SavedConnection::credentials);
        if let Some(creds) = found {
            break creds;
        }
        if Instant::now() >= deadline {
            return Err(DriverError::UnknownConnection(name));
        }
        tokio::time::sleep(REGISTRY_POLL).await;
    };
    h.service
        .connect(name, &creds)
        .await
        .map_err(|e| DriverError::Connect {
            connection: name,
            source: Box::new(e),
        })
}
