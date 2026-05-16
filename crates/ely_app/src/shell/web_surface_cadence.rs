use std::time::{Duration, Instant};

pub(super) const ACTIVE_POLL_INTERVAL: Duration = Duration::from_millis(8);
pub(super) const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(80);
const LOAD_BOOST_WINDOW: Duration = Duration::from_secs(5);
const INPUT_BOOST_WINDOW: Duration = Duration::from_millis(600);
const HOVER_BOOST_WINDOW: Duration = Duration::from_millis(120);
const FRAME_SETTLE_WINDOW: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default)]
pub(super) struct WebSurfacePollCadence {
    next_poll_at: Option<Instant>,
    load_active_until: Option<Instant>,
    interaction_active_until: Option<Instant>,
    settle_active_until: Option<Instant>,
    last_render_phase: Option<WebSurfaceRenderPhase>,
}

impl WebSurfacePollCadence {
    pub(super) fn note_ensure(
        &mut self,
        input_kind: WebSurfaceInputKind,
        started_loading: bool,
        now: Instant,
    ) {
        if started_loading {
            self.last_render_phase = None;
            self.settle_active_until = None;
            extend_deadline(&mut self.load_active_until, now + LOAD_BOOST_WINDOW);
        }
        match input_kind {
            WebSurfaceInputKind::Idle => {}
            WebSurfaceInputKind::Hover => {
                extend_deadline(&mut self.interaction_active_until, now + HOVER_BOOST_WINDOW);
            }
            WebSurfaceInputKind::Scroll
            | WebSurfaceInputKind::Click
            | WebSurfaceInputKind::Text => {
                extend_deadline(&mut self.interaction_active_until, now + INPUT_BOOST_WINDOW);
            }
        }
    }

    pub(super) fn note_frame(&mut self, render_state: &str, now: Instant) {
        let phase = WebSurfaceRenderPhase::from_render_state(render_state);
        match phase {
            WebSurfaceRenderPhase::Created | WebSurfaceRenderPhase::Loading
                if self.last_render_phase != Some(phase) =>
            {
                extend_deadline(&mut self.load_active_until, now + LOAD_BOOST_WINDOW);
            }
            WebSurfaceRenderPhase::Created | WebSurfaceRenderPhase::Loading => {}
            WebSurfaceRenderPhase::Complete if self.last_render_phase != Some(phase) => {
                self.load_active_until = None;
                extend_deadline(&mut self.settle_active_until, now + FRAME_SETTLE_WINDOW);
            }
            WebSurfaceRenderPhase::Complete => {}
            WebSurfaceRenderPhase::Other if self.last_render_phase != Some(phase) => {
                self.load_active_until = None;
                extend_deadline(&mut self.interaction_active_until, now + INPUT_BOOST_WINDOW);
            }
            WebSurfaceRenderPhase::Other => {}
        }
        self.last_render_phase = Some(phase);
    }

    pub(super) fn should_poll(&self, now: Instant) -> bool {
        self.next_poll_at.is_none_or(|next| now >= next)
    }

    pub(super) fn next_poll_delay(&self, now: Instant) -> Duration {
        self.next_poll_at.map_or(Duration::ZERO, |next| next.saturating_duration_since(now))
    }

    pub(super) fn note_poll_submitted(&mut self, now: Instant) {
        self.next_poll_at = Some(now + self.current_interval(now));
    }

    fn current_interval(&self, now: Instant) -> Duration {
        if self.has_active_window(now) { ACTIVE_POLL_INTERVAL } else { IDLE_POLL_INTERVAL }
    }

    fn has_active_window(&self, now: Instant) -> bool {
        [self.load_active_until, self.interaction_active_until, self.settle_active_until]
            .into_iter()
            .flatten()
            .any(|deadline| now < deadline)
    }
}

fn extend_deadline(slot: &mut Option<Instant>, deadline: Instant) {
    if slot.is_none_or(|current| deadline > current) {
        *slot = Some(deadline);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WebSurfaceRenderPhase {
    Created,
    Loading,
    Complete,
    Other,
}

impl WebSurfaceRenderPhase {
    fn from_render_state(render_state: &str) -> Self {
        match render_state {
            "created" => Self::Created,
            "loading" => Self::Loading,
            "complete" => Self::Complete,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WebSurfaceInputKind {
    Idle,
    Scroll,
    Click,
    Hover,
    Text,
}

impl WebSurfaceInputKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Scroll => "scroll",
            Self::Click => "click",
            Self::Hover => "hover",
            Self::Text => "text",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        ACTIVE_POLL_INTERVAL, IDLE_POLL_INTERVAL, WebSurfaceInputKind, WebSurfacePollCadence,
    };

    const FRAME_BUDGET_120HZ: Duration = Duration::from_micros(8_333);

    #[test]
    fn active_poll_interval_fits_120hz_budget() {
        assert!(ACTIVE_POLL_INTERVAL <= FRAME_BUDGET_120HZ);
    }

    #[test]
    fn active_inputs_hold_120hz_poll_cadence() {
        for input_kind in [
            WebSurfaceInputKind::Scroll,
            WebSurfaceInputKind::Click,
            WebSurfaceInputKind::Text,
            WebSurfaceInputKind::Hover,
        ] {
            let start = Instant::now();
            let mut cadence = WebSurfacePollCadence::default();

            cadence.note_ensure(input_kind, false, start);
            cadence.note_poll_submitted(start);

            assert!(cadence.next_poll_delay(start) <= FRAME_BUDGET_120HZ);
        }
    }

    #[test]
    fn loading_frames_hold_120hz_poll_cadence() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_frame("loading", start);
        cadence.note_poll_submitted(start);

        assert!(cadence.next_poll_delay(start) <= FRAME_BUDGET_120HZ);
    }

    #[test]
    fn idle_poll_uses_low_frequency_after_submission() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_poll_submitted(start);

        assert_eq!(cadence.next_poll_delay(start), IDLE_POLL_INTERVAL);
        assert!(!cadence.should_poll(start + Duration::from_millis(79)));
        assert!(cadence.should_poll(start + Duration::from_millis(80)));
    }

    #[test]
    fn scroll_input_uses_active_frame_cadence() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_ensure(WebSurfaceInputKind::Scroll, false, start);
        cadence.note_poll_submitted(start);

        assert_eq!(cadence.next_poll_delay(start), ACTIVE_POLL_INTERVAL);
        assert!(!cadence.should_poll(start + Duration::from_millis(7)));
        assert!(cadence.should_poll(start + Duration::from_millis(8)));
    }

    #[test]
    fn complete_frames_settle_then_return_to_idle_cadence() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_frame("complete", start);
        cadence.note_poll_submitted(start + Duration::from_millis(300));

        assert!(!cadence.should_poll(start + Duration::from_millis(379)));
        assert!(cadence.should_poll(start + Duration::from_millis(380)));
    }

    #[test]
    fn complete_frame_shortens_started_loading_boost() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_ensure(WebSurfaceInputKind::Idle, true, start);
        cadence.note_frame("complete", start + Duration::from_secs(1));
        cadence.note_poll_submitted(start + Duration::from_millis(1_300));

        assert_eq!(
            cadence.next_poll_delay(start + Duration::from_millis(1_300)),
            IDLE_POLL_INTERVAL
        );
    }

    #[test]
    fn complete_frame_preserves_recent_input_boost() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_ensure(WebSurfaceInputKind::Scroll, false, start);
        cadence.note_frame("complete", start + Duration::from_millis(100));
        cadence.note_poll_submitted(start + Duration::from_millis(500));

        assert_eq!(
            cadence.next_poll_delay(start + Duration::from_millis(500)),
            ACTIVE_POLL_INTERVAL
        );
    }

    #[test]
    fn repeated_complete_frames_do_not_extend_settle_window() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_frame("complete", start);
        cadence.note_frame("complete", start + Duration::from_millis(200));
        cadence.note_poll_submitted(start + Duration::from_millis(260));

        assert!(!cadence.should_poll(start + Duration::from_millis(339)));
        assert!(cadence.should_poll(start + Duration::from_millis(340)));
    }

    #[test]
    fn repeated_loading_frames_do_not_extend_load_boost_window() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_frame("loading", start);
        cadence.note_frame("loading", start + Duration::from_millis(200));
        cadence.note_poll_submitted(start + Duration::from_millis(5_010));

        assert_eq!(
            cadence.next_poll_delay(start + Duration::from_millis(5_010)),
            IDLE_POLL_INTERVAL
        );
    }

    #[test]
    fn repeated_other_frames_do_not_extend_active_window() {
        let start = Instant::now();
        let mut cadence = WebSurfacePollCadence::default();

        cadence.note_frame("sleeping", start);
        cadence.note_frame("sleeping", start + Duration::from_millis(200));
        cadence.note_poll_submitted(start + Duration::from_millis(610));

        assert_eq!(cadence.next_poll_delay(start + Duration::from_millis(610)), IDLE_POLL_INTERVAL);
    }
}
