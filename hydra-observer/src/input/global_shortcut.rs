//! Global shortcut handling via xdg-desktop-portal GlobalShortcuts
//!
//! Uses the proper portal API for registering global keyboard shortcuts.

use anyhow::{Context, Result};
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use ashpd::WindowIdentifier;
use futures_util::StreamExt;
use std::sync::mpsc;
use std::thread;

/// Events from global shortcut
#[derive(Debug, Clone, Copy)]
pub enum ShortcutEvent {
    /// Toggle drag mode
    ToggleDrag,
}

/// Spawn a thread that listens for global shortcuts via the portal
pub fn spawn_global_shortcut_listener() -> Option<mpsc::Receiver<ShortcutEvent>> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create tokio runtime: {}", e);
                return;
            }
        };

        rt.block_on(async {
            if let Err(e) = run_shortcut_listener(tx).await {
                tracing::error!("Global shortcut listener failed: {}", e);
            }
        });
    });

    Some(rx)
}

async fn run_shortcut_listener(tx: mpsc::Sender<ShortcutEvent>) -> Result<()> {
    tracing::info!("Starting GlobalShortcuts portal listener...");

    let gs = GlobalShortcuts::new()
        .await
        .context("Failed to connect to GlobalShortcuts portal")?;

    let session = gs
        .create_session()
        .await
        .context("Failed to create global shortcuts session")?;

    // Register our toggle shortcut (Alt+Shift+H)
    let shortcut = NewShortcut::new("toggle_drag", "Hydra: Toggle Mascot Drag")
        .preferred_trigger(Some("ALT+SHIFT+H"));

    let parent_window = WindowIdentifier::default();
    let request = gs
        .bind_shortcuts(&session, &[shortcut], &parent_window)
        .await
        .context("Failed to bind shortcuts")?;

    let response = request
        .response()
        .context("Failed to get bind shortcuts response")?;

    // Check what was bound
    let shortcuts = response.shortcuts();
    let bound_exists = shortcuts.iter().any(|s| s.id() == "toggle_drag");

    if bound_exists {
        tracing::info!("Global shortcut registered: Alt+Shift+H to toggle drag");
    } else {
        tracing::warn!("Shortcut not bound - user may have declined or portal rejected");
        return Err(anyhow::anyhow!("Shortcut binding not approved"));
    }

    // Listen for activations
    let mut activated_stream = gs
        .receive_activated()
        .await
        .context("Failed to subscribe to Activated signal")?;

    loop {
        tokio::select! {
            Some(activated) = activated_stream.next() => {
                if activated.shortcut_id() == "toggle_drag" {
                    tracing::info!("Toggle drag shortcut activated!");
                    if tx.send(ShortcutEvent::ToggleDrag).is_err() {
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Keep the session alive
            }
        }
    }

    Ok(())
}
