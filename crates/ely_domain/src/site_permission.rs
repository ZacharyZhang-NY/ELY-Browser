use std::time::SystemTime;

use url::Url;

use crate::{DomainError, ProfileId, UrlText};

pub const MAX_SITE_ORIGIN_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SiteOrigin(String);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SitePermissionFeature {
    Camera,
    Microphone,
    ScreenCapture,
    Location,
    Notifications,
    ClipboardRead,
    ClipboardWrite,
    Downloads,
    Popups,
    Autoplay,
    WebUsb,
    WebHid,
    WebSerial,
    StoragePersistence,
    InsecureContent,
    CertificateException,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SitePermissionDecision {
    AllowOnce,
    AllowAlways,
    DenyAlways,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SitePermissionEntry {
    profile_id: ProfileId,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    decision: SitePermissionDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SitePermissionAuditAction {
    Set(SitePermissionDecision),
    Revoked,
    Transferred,
    Consumed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SitePermissionAuditEvent {
    profile_id: ProfileId,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    action: SitePermissionAuditAction,
    created_at: SystemTime,
}

impl SiteOrigin {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField { field: "site_origin" });
        }

        let url = Url::parse(trimmed)
            .map_err(|_| DomainError::InvalidSiteOrigin { value: trimmed.to_string() })?;
        site_origin_from_url(&url)
    }

    pub fn from_url(url: &UrlText) -> Result<Option<Self>, DomainError> {
        let parsed = Url::parse(url.as_str())
            .map_err(|_| DomainError::InvalidUrl { value: url.as_str().to_string() })?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Ok(None);
        }

        site_origin_from_url(&parsed).map(Some)
    }

    pub fn from_site_route(route: &str) -> Result<Option<Self>, DomainError> {
        let Some(origin) = route.strip_prefix("ely://site/") else {
            return Ok(None);
        };
        Self::parse(origin).map(Some)
    }

    pub fn site_settings_url(&self) -> Result<UrlText, DomainError> {
        UrlText::parse(format!("ely://site/{}", self.as_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SitePermissionFeature {
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Camera,
            Self::Microphone,
            Self::ScreenCapture,
            Self::Location,
            Self::Notifications,
            Self::ClipboardRead,
            Self::ClipboardWrite,
            Self::Downloads,
            Self::Popups,
            Self::Autoplay,
            Self::WebUsb,
            Self::WebHid,
            Self::WebSerial,
            Self::StoragePersistence,
            Self::InsecureContent,
            Self::CertificateException,
        ]
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::ScreenCapture => "screen-capture",
            Self::Location => "location",
            Self::Notifications => "notifications",
            Self::ClipboardRead => "clipboard-read",
            Self::ClipboardWrite => "clipboard-write",
            Self::Downloads => "downloads",
            Self::Popups => "popups",
            Self::Autoplay => "autoplay",
            Self::WebUsb => "webusb",
            Self::WebHid => "webhid",
            Self::WebSerial => "webserial",
            Self::StoragePersistence => "storage-persistence",
            Self::InsecureContent => "insecure-content",
            Self::CertificateException => "certificate-exception",
        }
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "camera" => Ok(Self::Camera),
            "microphone" => Ok(Self::Microphone),
            "screen-capture" => Ok(Self::ScreenCapture),
            "location" => Ok(Self::Location),
            "notifications" => Ok(Self::Notifications),
            "clipboard-read" => Ok(Self::ClipboardRead),
            "clipboard-write" => Ok(Self::ClipboardWrite),
            "downloads" => Ok(Self::Downloads),
            "popups" => Ok(Self::Popups),
            "autoplay" => Ok(Self::Autoplay),
            "webusb" => Ok(Self::WebUsb),
            "webhid" => Ok(Self::WebHid),
            "webserial" => Ok(Self::WebSerial),
            "storage-persistence" => Ok(Self::StoragePersistence),
            "insecure-content" => Ok(Self::InsecureContent),
            "certificate-exception" => Ok(Self::CertificateException),
            _ => Err(DomainError::InvalidSitePermissionFeature { value: value.to_string() }),
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Camera => "Camera",
            Self::Microphone => "Microphone",
            Self::ScreenCapture => "Screen capture",
            Self::Location => "Location",
            Self::Notifications => "Notifications",
            Self::ClipboardRead => "Clipboard read",
            Self::ClipboardWrite => "Clipboard write",
            Self::Downloads => "Downloads",
            Self::Popups => "Popups",
            Self::Autoplay => "Autoplay",
            Self::WebUsb => "WebUSB",
            Self::WebHid => "WebHID",
            Self::WebSerial => "WebSerial",
            Self::StoragePersistence => "Storage persistence",
            Self::InsecureContent => "Insecure content",
            Self::CertificateException => "Certificate exception",
        }
    }
}

impl SitePermissionDecision {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllowOnce => "allow-once",
            Self::AllowAlways => "allow-always",
            Self::DenyAlways => "deny-always",
        }
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "allow-once" => Ok(Self::AllowOnce),
            "allow-always" => Ok(Self::AllowAlways),
            "deny-always" => Ok(Self::DenyAlways),
            _ => Err(DomainError::InvalidSitePermissionDecision { value: value.to_string() }),
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::AllowOnce => "Allow once",
            Self::AllowAlways => "Allow always",
            Self::DenyAlways => "Deny always",
        }
    }
}

impl SitePermissionEntry {
    #[must_use]
    pub fn new(
        profile_id: ProfileId,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        decision: SitePermissionDecision,
    ) -> Self {
        Self { profile_id, origin, feature, decision }
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn origin(&self) -> &SiteOrigin {
        &self.origin
    }

    #[must_use]
    pub fn feature(&self) -> SitePermissionFeature {
        self.feature
    }

    #[must_use]
    pub fn decision(&self) -> SitePermissionDecision {
        self.decision
    }

    pub fn set_decision(&mut self, decision: SitePermissionDecision) {
        self.decision = decision;
    }
}

impl SitePermissionAuditEvent {
    #[must_use]
    pub fn new(
        profile_id: ProfileId,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        action: SitePermissionAuditAction,
        created_at: SystemTime,
    ) -> Self {
        Self { profile_id, origin, feature, action, created_at }
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn origin(&self) -> &SiteOrigin {
        &self.origin
    }

    #[must_use]
    pub fn feature(&self) -> SitePermissionFeature {
        self.feature
    }

    #[must_use]
    pub fn action(&self) -> &SitePermissionAuditAction {
        &self.action
    }

    #[must_use]
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }
}

fn site_origin_from_url(url: &Url) -> Result<SiteOrigin, DomainError> {
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(DomainError::InvalidSiteOrigin { value: url.to_string() });
    }

    let origin = url.origin().ascii_serialization();
    if origin.len() > MAX_SITE_ORIGIN_BYTES {
        return Err(DomainError::InvalidSiteOrigin { value: origin });
    }
    Ok(SiteOrigin(origin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_site_origin_accepts_512_bytes_and_rejects_513() -> Result<(), DomainError> {
        let exact = SiteOrigin::parse(format!("https://{}", ascii_host(504)))?;
        assert_eq!(exact.as_str().len(), MAX_SITE_ORIGIN_BYTES);

        assert!(matches!(
            SiteOrigin::parse(format!("https://{}", ascii_host(505))),
            Err(DomainError::InvalidSiteOrigin { .. })
        ));
        Ok(())
    }

    fn ascii_host(mut bytes: usize) -> String {
        let mut labels = Vec::new();
        while bytes > 63 {
            labels.push("a".repeat(63));
            bytes -= 64;
        }
        labels.push("a".repeat(bytes));
        labels.join(".")
    }
}
