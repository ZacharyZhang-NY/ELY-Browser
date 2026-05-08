use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use servo::EventLoopWaker;

#[derive(Clone)]
pub(super) struct ServoWakeFlag {
    requested: Arc<AtomicBool>,
}

impl ServoWakeFlag {
    pub(super) fn new(requested: Arc<AtomicBool>) -> Self {
        Self { requested }
    }
}

impl EventLoopWaker for ServoWakeFlag {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        self.requested.store(true, Ordering::Release);
    }
}
