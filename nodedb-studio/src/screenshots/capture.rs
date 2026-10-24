//! Writes the window's current pixels to a PNG, as WebKitGTK rendered them.

use std::fs::File;
use std::path::{Path, PathBuf};

use dioxus::desktop::wry::WebViewExtUnix;
use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("WebKitGTK snapshot error: {0}")]
    Snapshot(#[from] webkit2gtk::glib::Error),
    #[error("cannot create {}: {source}", path.display())]
    Create {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write PNG {}: {source}", path.display())]
    Encode {
        path: PathBuf,
        source: cairo::IoError,
    },
}

/// Snapshot the visible viewport at the window's device scale and write it to
/// `path`. Must run on the UI thread, which is where Dioxus polls its tasks.
pub async fn capture(path: &Path) -> Result<(), CaptureError> {
    let webview = dioxus::desktop::window().webview.webview();
    let surface = webview
        .snapshot_future(SnapshotRegion::Visible, SnapshotOptions::NONE)
        .await?;
    let mut file = File::create(path).map_err(|source| CaptureError::Create {
        path: path.to_path_buf(),
        source,
    })?;
    surface
        .write_to_png(&mut file)
        .map_err(|source| CaptureError::Encode {
            path: path.to_path_buf(),
            source,
        })
}
