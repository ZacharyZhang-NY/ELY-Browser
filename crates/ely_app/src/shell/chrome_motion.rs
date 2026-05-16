use gpui::SharedString;

use super::{ElyShell, ShellState};

#[derive(Default)]
pub(crate) struct ChromeMotionState {
    target: Option<SharedString>,
    epoch: u64,
}

impl ElyShell {
    pub(crate) fn chrome_motion_enabled(&self) -> bool {
        match &self.state {
            ShellState::Ready(core) => !core.appearance().reduce_motion(),
            ShellState::StartupError(_) => true,
        }
    }

    pub(crate) fn trigger_chrome_motion(&mut self, target: impl Into<SharedString>) {
        if !self.chrome_motion_enabled() {
            self.chrome_motion.target = None;
            return;
        }

        self.chrome_motion.target = Some(target.into());
        self.chrome_motion.epoch = self.chrome_motion.epoch.wrapping_add(1);
    }

    pub(crate) fn chrome_motion_animation_id(&self, target: &str) -> Option<SharedString> {
        if !self.chrome_motion_enabled() {
            return None;
        }

        self.chrome_motion
            .target
            .as_ref()
            .filter(|current| current.as_str() == target)
            .map(|_| SharedString::from(format!("{target}-motion-{}", self.chrome_motion.epoch)))
    }
}
