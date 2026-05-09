use std::time::SystemTime;

use ely_domain::{DiagnosticEvent, DiagnosticEventKind};

use super::BrowserCore;

impl BrowserCore {
    pub fn record_diagnostic_event(&mut self, kind: DiagnosticEventKind) {
        self.diagnostic_events.push(DiagnosticEvent::new(kind, SystemTime::now()));
    }

    #[must_use]
    pub fn diagnostic_events(&self) -> &[DiagnosticEvent] {
        &self.diagnostic_events
    }
}
