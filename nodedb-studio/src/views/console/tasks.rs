//! The Console's writes. They run under `spawn_forever` and touch root
//! state only, so a command finishes when the user leaves the page.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::models::console::Protocol;
use crate::services::backend::Backend;
use crate::state::console::ConsoleState;
use crate::state::query::TxEpoch;

#[derive(Clone, Copy)]
pub struct Env {
    pub state: Signal<ConsoleState>,
    pub tx: TxEpoch,
}

/// Run `text` in `protocol` against `database`, and record the outcome in
/// the transcript. `clear_prompt` empties the prompt when the command ran.
async fn run(
    env: Env,
    backend: Rc<dyn Backend>,
    protocol: Protocol,
    database: String,
    text: String,
    clear_prompt: bool,
) {
    let mut state = env.state;
    state.write().busy = true;
    let out = backend.console_run(protocol, &database, &text).await;
    {
        let mut s = state.write();
        s.busy = false;
        match out {
            Ok(reply) => {
                if clear_prompt {
                    s.edit_prompt(String::new());
                }
                s.push(protocol, &text, Ok(reply));
            }
            Err(e) => s.push(protocol, &text, Err(e.to_string())),
        }
    }
    let mut epoch = env.tx.0;
    let next = *epoch.peek() + 1;
    epoch.set(next);
}

/// Run the prompt. An empty prompt does nothing.
pub fn start_prompt(env: Env, backend: Rc<dyn Backend>, session_db: &str) {
    let (protocol, database, text, busy) = {
        let s = env.state.peek();
        (
            s.protocol,
            s.database.clone().unwrap_or_else(|| session_db.to_string()),
            s.prompt.clone(),
            s.busy,
        )
    };
    if busy || text.trim().is_empty() {
        return;
    }
    spawn_forever(async move { run(env, backend, protocol, database, text, true).await });
}

/// Run an earlier command again, in its own protocol.
pub fn start_rerun(
    env: Env,
    backend: Rc<dyn Backend>,
    session_db: &str,
    protocol: Protocol,
    text: String,
) {
    let database = {
        let mut state = env.state;
        let mut s = state.write();
        if s.busy {
            return;
        }
        s.set_protocol(protocol);
        s.database.clone().unwrap_or_else(|| session_db.to_string())
    };
    spawn_forever(async move { run(env, backend, protocol, database, text, false).await });
}
