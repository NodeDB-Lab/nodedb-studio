//! Copy text to the system clipboard through the webview.

use dioxus::prelude::*;

#[derive(Debug, Clone, thiserror::Error)]
pub enum CopyError {
    #[error("the webview did not accept the text: {0}")]
    Send(String),
    #[error("the clipboard script did not finish: {0}")]
    Script(String),
    #[error("the system clipboard refused the text")]
    Refused,
}

/// Runs in the webview. It tries the async clipboard API, then falls back
/// to a hidden text area and `execCommand`, which a non-secure origin allows.
const SCRIPT: &str = r#"
const text = await dioxus.recv();
try {
  await navigator.clipboard.writeText(text);
  return true;
} catch (_) {}
const area = document.createElement('textarea');
area.value = text;
area.style.position = 'fixed';
area.style.opacity = '0';
document.body.appendChild(area);
area.select();
let ok = false;
try { ok = document.execCommand('copy'); } catch (_) {}
area.remove();
return ok;
"#;

/// Copy `text`. The text goes to the script as a message, never spliced into
/// the script source.
pub async fn copy_text(text: String) -> Result<(), CopyError> {
    let eval = document::eval(SCRIPT);
    eval.send(text)
        .map_err(|e| CopyError::Send(e.to_string()))?;
    match eval.join::<bool>().await {
        Ok(true) => Ok(()),
        Ok(false) => Err(CopyError::Refused),
        Err(e) => Err(CopyError::Script(e.to_string())),
    }
}
