//! Shell glue for on-disk browser state: restore at construction, a
//! debounced save after every mutation that schedules a sync upload, and
//! a final synchronous save when the app quits.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use ely_browser_core::BrowserCore;
use ely_domain::ProfileId;
use gpui::{Context, Subscription, Timer};

use super::{ElyShell, ShellState};
use crate::services::local_state::{
    load_local_state, local_state_path, quarantine_local_state, save_local_state,
};
use crate::services::servo_profile_data::default_profile_data_root;

const LOCAL_STATE_SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

pub(super) fn resolve_local_state_path(default_profile_id: Option<&ProfileId>) -> Option<PathBuf> {
    // Harness tests build the real shell; persistence stays inert there so
    // tests never read or write the developer's actual profile.
    if cfg!(test) {
        return None;
    }
    let profile_id = default_profile_id?;
    let root = default_profile_data_root()?;
    Some(local_state_path(&root, profile_id))
}

/// Restore persisted state into a freshly constructed core. A corrupt or
/// unreadable file is quarantined loudly instead of silently replaced, so
/// the previous state stays recoverable for diagnosis.
pub(super) fn restore_local_state(core: &mut BrowserCore, path: &Path) {
    let bytes = match load_local_state(path) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return,
        Err(error) => {
            tracing::error!(
                target: "ely::local_state",
                error = %error,
                path = %path.display(),
                "local state read failed; starting from defaults",
            );
            return;
        }
    };
    match core.apply_local_state_bytes(&bytes) {
        Ok(()) => {
            tracing::info!(
                target: "ely::local_state",
                path = %path.display(),
                bytes = bytes.len(),
                "local state restored",
            );
        }
        Err(error) => {
            tracing::error!(
                target: "ely::local_state",
                error = %error,
                path = %path.display(),
                "local state restore failed; quarantining the file",
            );
            match quarantine_local_state(path) {
                Ok(quarantined) => tracing::warn!(
                    target: "ely::local_state",
                    path = %quarantined.display(),
                    "corrupt local state preserved for diagnosis",
                ),
                Err(error) => tracing::error!(
                    target: "ely::local_state",
                    error = %error,
                    "local state quarantine failed",
                ),
            }
        }
    }
}

pub(super) fn register_quit_save(cx: &mut Context<ElyShell>) -> Subscription {
    cx.on_app_quit(|shell, _cx| {
        shell.save_local_state_blocking();
        async {}
    })
}

impl ElyShell {
    /// Every mutation that schedules a cloud sync upload also schedules a
    /// local save; the debounce collapses bursts into one write.
    pub(crate) fn schedule_local_state_save(&mut self, cx: &mut Context<Self>) {
        if self.local_state_path.is_none() || self.local_state_save_scheduled {
            return;
        }
        self.local_state_save_scheduled = true;
        cx.spawn(async move |shell, cx| {
            Timer::after(LOCAL_STATE_SAVE_DEBOUNCE).await;
            let _ = shell.update(cx, |shell, _| {
                shell.local_state_save_scheduled = false;
                shell.save_local_state_in_background();
            });
        })
        .detach();
    }

    fn save_local_state_in_background(&self) {
        let Some((path, bytes)) = self.build_local_state_write() else {
            return;
        };
        std::thread::Builder::new()
            .name("ely-local-state-save".to_string())
            .spawn(move || {
                if let Err(error) = save_local_state(&path, &bytes) {
                    tracing::error!(
                        target: "ely::local_state",
                        error = %error,
                        path = %path.display(),
                        "local state save failed",
                    );
                }
            })
            .map(|_| ())
            .unwrap_or_else(|error| {
                tracing::warn!(
                    target: "ely::local_state",
                    error = %error,
                    "spawn local state save failed",
                );
            });
    }

    pub(super) fn save_local_state_blocking(&self) {
        let Some((path, bytes)) = self.build_local_state_write() else {
            return;
        };
        if let Err(error) = save_local_state(&path, &bytes) {
            tracing::error!(
                target: "ely::local_state",
                error = %error,
                path = %path.display(),
                "local state save on quit failed",
            );
        }
    }

    fn build_local_state_write(&self) -> Option<(PathBuf, Vec<u8>)> {
        let path = self.local_state_path.clone()?;
        let ShellState::Ready(core) = &self.state else {
            return None;
        };
        match core.build_local_state_bytes() {
            Ok(bytes) => Some((path, bytes)),
            Err(error) => {
                tracing::error!(
                    target: "ely::local_state",
                    error = %error,
                    "local state serialization failed",
                );
                None
            }
        }
    }
}
