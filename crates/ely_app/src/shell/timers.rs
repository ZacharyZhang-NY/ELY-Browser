//! Long-lived shell timers, started once per window at construction.

use std::time::Duration;

use gpui::{Context, Timer};

use super::{ElyShell, ShellState};

/// Idle tabs archive on this cadence when the active Space opts into an
/// idle policy; the sweep is a no-op for `ArchivePolicy::Manual`.
const IDLE_ARCHIVE_SWEEP_INTERVAL: Duration = Duration::from_secs(30 * 60);

pub(super) fn start(cx: &mut Context<ElyShell>) {
    start_external_web_surface_timer(cx);
    start_idle_archive_timer(cx);
}

fn start_external_web_surface_timer(cx: &mut Context<ElyShell>) {
    cx.spawn(async move |shell, cx| {
        loop {
            let delay = match shell.update(cx, |shell, _| shell.external_web_surface_tick_delay()) {
                Ok(delay) => delay,
                Err(_) => break,
            };
            Timer::after(delay).await;
            let result = shell.update(cx, |shell, cx| {
                if shell.tick_external_web_surfaces(cx) {
                    cx.notify();
                }
            });
            if result.is_err() {
                break;
            }
        }
    })
    .detach();
}

fn start_idle_archive_timer(cx: &mut Context<ElyShell>) {
    cx.spawn(async move |shell, cx| {
        loop {
            Timer::after(IDLE_ARCHIVE_SWEEP_INTERVAL).await;
            let result = shell.update(cx, |shell, cx| {
                let ShellState::Ready(core) = &mut shell.state else {
                    return;
                };
                match core.archive_idle_tabs(std::time::SystemTime::now()) {
                    Ok(0) => {}
                    Ok(archived) => {
                        tracing::info!(
                            target: "ely::archive",
                            archived,
                            "idle archive sweep archived tabs",
                        );
                        shell.schedule_cloud_sync_upload(cx);
                        cx.notify();
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "ely::archive",
                            error = %error,
                            "idle archive sweep failed",
                        );
                    }
                }
            });
            if result.is_err() {
                break;
            }
        }
    })
    .detach();
}
