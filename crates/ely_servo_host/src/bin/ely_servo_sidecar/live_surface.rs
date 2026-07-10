use std::collections::{HashMap, HashSet};

use ely_servo_host::{IOSurfaceIdentity, SoftwareServoHost};

use super::{
    iosurface_mach::IOSurfaceMachSender,
    live_protocol::{LiveOutcome, LiveSidecarError},
};

pub(super) struct HardwareSurfaceTransport {
    sender: IOSurfaceMachSender,
    publications: SurfacePublications,
}

#[derive(Default)]
struct SurfacePublications {
    by_tab: HashMap<String, HashMap<IOSurfaceIdentity, PublicationState>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationState {
    AwaitingReady,
    Ready,
}

impl HardwareSurfaceTransport {
    pub(super) fn connect(service_name: &str) -> Result<Self, LiveSidecarError> {
        Ok(Self {
            sender: IOSurfaceMachSender::connect(service_name)?,
            publications: SurfacePublications::default(),
        })
    }

    pub(super) fn publish_frame(
        &mut self,
        host: &SoftwareServoHost,
        tab_id: &str,
        webview_id: &ely_domain::WebViewId,
        ready_surface_ids: &[u64],
        pending_surface_ids: &[u64],
        outcome: &mut LiveOutcome,
    ) -> Result<(), LiveSidecarError> {
        self.publications.sync_client_state(tab_id, ready_surface_ids, pending_surface_ids);
        let Some(report) = outcome.response.frame.as_ref() else {
            return Ok(());
        };
        let identity = host.peek_iosurface_identity(webview_id)?.ok_or_else(|| {
            ely_servo_host::ServoHostError::HardwareSurfaceUnavailable { id: webview_id.clone() }
        })?;
        validate_report(identity, report.width, report.height)?;
        outcome.response.current_surface_id = Some(identity.surface_id);
        if self.publications.contains(tab_id, identity) {
            return Ok(());
        }

        let handle = host.current_iosurface_handle(webview_id)?.ok_or_else(|| {
            ely_servo_host::ServoHostError::HardwareSurfaceUnavailable { id: webview_id.clone() }
        })?;
        if IOSurfaceIdentity::from_handle(handle) != identity {
            return Err(LiveSidecarError::HardwareSurfaceHandleMismatch);
        }
        self.sender.send_surface_port(handle.surface_id, handle.mach_port_name)?;
        outcome.response.surface_handle = Some(handle);
        self.publications.insert(tab_id, identity);
        Ok(())
    }

    pub(super) fn close_tab(&mut self, tab_id: &str) {
        self.publications.remove(tab_id);
    }
}

impl SurfacePublications {
    fn sync_client_state(
        &mut self,
        tab_id: &str,
        ready_surface_ids: &[u64],
        pending_surface_ids: &[u64],
    ) {
        let ready: HashSet<u64> = ready_surface_ids.iter().copied().collect();
        let pending: HashSet<u64> = pending_surface_ids.iter().copied().collect();
        let Some(surfaces) = self.by_tab.get_mut(tab_id) else {
            return;
        };
        surfaces.retain(|identity, state| match state {
            PublicationState::AwaitingReady => {
                if ready.contains(&identity.surface_id) {
                    *state = PublicationState::Ready;
                }
                ready.contains(&identity.surface_id) || pending.contains(&identity.surface_id)
            }
            PublicationState::Ready => ready.contains(&identity.surface_id),
        });
    }

    fn contains(&self, tab_id: &str, identity: IOSurfaceIdentity) -> bool {
        self.by_tab.get(tab_id).is_some_and(|surfaces| surfaces.contains_key(&identity))
    }

    fn insert(&mut self, tab_id: &str, identity: IOSurfaceIdentity) {
        self.by_tab
            .entry(tab_id.to_string())
            .or_default()
            .insert(identity, PublicationState::AwaitingReady);
    }

    fn remove(&mut self, tab_id: &str) {
        self.by_tab.remove(tab_id);
    }
}

fn validate_report(
    identity: IOSurfaceIdentity,
    frame_width: u32,
    frame_height: u32,
) -> Result<(), LiveSidecarError> {
    if identity.width == frame_width && identity.height == frame_height {
        return Ok(());
    }
    Err(LiveSidecarError::HardwareSurfaceReportMismatch {
        surface_id: identity.surface_id,
        surface_width: identity.width,
        surface_height: identity.height,
        frame_width,
        frame_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_publication_tracks_pending_ready_and_evicted_states() {
        let identity = IOSurfaceIdentity { surface_id: 7, width: 800, height: 600 };
        let mut publications = SurfacePublications::default();
        publications.insert("tab", identity);

        publications.sync_client_state("tab", &[], &[7]);
        assert!(publications.contains("tab", identity));
        publications.sync_client_state("tab", &[7], &[]);
        assert!(publications.contains("tab", identity));
        publications.sync_client_state("tab", &[], &[]);
        assert!(!publications.contains("tab", identity));

        publications.insert("tab", identity);
        publications.sync_client_state("tab", &[], &[7]);
        assert!(publications.contains("tab", identity));
        publications.sync_client_state("tab", &[7], &[]);
        assert!(publications.contains("tab", identity));
    }

    #[test]
    fn seventeenth_surface_missing_before_ready_is_republished() {
        let mut publications = SurfacePublications::default();
        let identities = (1..=17)
            .map(|surface_id| IOSurfaceIdentity { surface_id, width: 64, height: 48 })
            .collect::<Vec<_>>();
        for identity in &identities {
            publications.insert("tab", *identity);
        }

        publications.sync_client_state("tab", &[], &(1..=17).collect::<Vec<_>>());
        publications.sync_client_state("tab", &[], &(2..=17).collect::<Vec<_>>());

        assert!(!publications.contains("tab", identities[0]));
        assert!(publications.contains("tab", identities[16]));
    }
}
