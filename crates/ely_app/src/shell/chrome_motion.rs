use gpui::SharedString;

use super::ElyShell;

#[derive(Default)]
pub(crate) struct ChromeMotionState {
    target: Option<SharedString>,
    epoch: u64,
}

impl ElyShell {
    pub(crate) fn trigger_chrome_motion(&mut self, target: impl Into<SharedString>) {
        self.chrome_motion.target = Some(target.into());
        self.chrome_motion.epoch = self.chrome_motion.epoch.wrapping_add(1);
    }

    pub(crate) fn chrome_motion_animation_id(&self, target: &str) -> Option<SharedString> {
        self.chrome_motion
            .target
            .as_ref()
            .filter(|current| current.as_str() == target)
            .map(|_| SharedString::from(format!("{target}-motion-{}", self.chrome_motion.epoch)))
    }
}
