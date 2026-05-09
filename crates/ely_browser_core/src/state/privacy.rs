use ely_domain::{DiagnosticsReportingPolicy, HistoryRecordingPolicy};

use super::BrowserCore;

impl BrowserCore {
    pub fn set_history_recording_policy(&mut self, policy: HistoryRecordingPolicy) {
        self.history_recording_policy = policy;
    }

    pub fn set_diagnostics_reporting_policy(&mut self, policy: DiagnosticsReportingPolicy) {
        self.diagnostics_reporting_policy = policy;
    }

    pub fn reset_privacy_settings(&mut self) {
        self.set_history_recording_policy(HistoryRecordingPolicy::default());
        self.set_diagnostics_reporting_policy(DiagnosticsReportingPolicy::default());
    }

    #[must_use]
    pub fn history_recording_policy(&self) -> HistoryRecordingPolicy {
        self.history_recording_policy
    }

    #[must_use]
    pub fn diagnostics_reporting_policy(&self) -> DiagnosticsReportingPolicy {
        self.diagnostics_reporting_policy
    }
}
