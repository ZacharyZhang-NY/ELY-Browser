use std::time::SystemTime;

use crate::{DomainError, PluginId};

const DIAGNOSTIC_CODE_MAX_LEN: usize = 80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOutcome {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebViewCrashKind {
    TabCrashed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCode(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticEventKind {
    AppStartup { outcome: DiagnosticOutcome },
    AppCrash,
    WebViewCrash { crash_kind: WebViewCrashKind },
    SyncError { error_code: DiagnosticCode },
    PluginCrash { plugin_id: PluginId },
    UpdateResult { outcome: DiagnosticOutcome },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    kind: DiagnosticEventKind,
    occurred_at: SystemTime,
}

impl DiagnosticOutcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

impl WebViewCrashKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TabCrashed => "tab_crashed",
        }
    }
}

impl DiagnosticCode {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty()
            || trimmed.len() > DIAGNOSTIC_CODE_MAX_LEN
            || !trimmed.chars().all(is_diagnostic_code_character)
        {
            return Err(DomainError::InvalidDiagnosticCode { value });
        }

        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl DiagnosticEventKind {
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AppStartup { .. } => "app_startup",
            Self::AppCrash => "app_crash",
            Self::WebViewCrash { .. } => "webview_crash",
            Self::SyncError { .. } => "sync_error",
            Self::PluginCrash { .. } => "plugin_crash",
            Self::UpdateResult { .. } => "update_result",
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::AppStartup { .. } => "App startup",
            Self::AppCrash => "App crash",
            Self::WebViewCrash { .. } => "WebView crash",
            Self::SyncError { .. } => "Sync error",
            Self::PluginCrash { .. } => "Plugin crash",
            Self::UpdateResult { .. } => "Update result",
        }
    }
}

impl DiagnosticEvent {
    #[must_use]
    pub fn new(kind: DiagnosticEventKind, occurred_at: SystemTime) -> Self {
        Self { kind, occurred_at }
    }

    #[must_use]
    pub fn startup_success(occurred_at: SystemTime) -> Self {
        Self::new(
            DiagnosticEventKind::AppStartup { outcome: DiagnosticOutcome::Success },
            occurred_at,
        )
    }

    #[must_use]
    pub fn kind(&self) -> &DiagnosticEventKind {
        &self.kind
    }

    #[must_use]
    pub fn occurred_at(&self) -> SystemTime {
        self.occurred_at
    }
}

fn is_diagnostic_code_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '-')
}

#[cfg(test)]
mod tests {
    use super::DiagnosticCode;

    #[test]
    fn diagnostic_code_accepts_machine_codes() -> Result<(), Box<dyn std::error::Error>> {
        let code = DiagnosticCode::parse("sync.pull.5xx")?;

        assert_eq!(code.as_str(), "sync.pull.5xx");
        Ok(())
    }

    #[test]
    fn diagnostic_code_rejects_url_like_values() {
        let result = DiagnosticCode::parse("https://example.com/private?q=token");

        assert!(result.is_err());
    }
}
