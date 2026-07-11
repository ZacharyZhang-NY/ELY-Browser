use std::time::{Duration, Instant};

use ely_domain::{ProfileId, TabId};

use crate::services::{ProfileDataMode, servo_live::ServoLiveFrame};

use super::{
    super::{
        web_surface_runtime_session::session_for_scope,
        web_surface_worker::{RequestGeneration, WorkerResponse},
    },
    SIDECAR_RESTART_BASE_DELAY, SIDECAR_RESTART_MAX_DELAY, ScopeRetryState, WebSurfaceRuntime,
    WebSurfaceRuntimeScope,
};

#[test]
fn retry_delay_grows_exponentially_and_caps() {
    let now = Instant::now();
    let expected = [
        Duration::from_millis(250),
        Duration::from_millis(500),
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(4),
        SIDECAR_RESTART_MAX_DELAY,
        SIDECAR_RESTART_MAX_DELAY,
    ];
    let mut state = None;

    for delay in expected {
        let next = ScopeRetryState::after_failure(state.as_ref(), now);
        assert_eq!(next.retry_after.duration_since(now), delay);
        state = Some(next);
    }
    assert_eq!(SIDECAR_RESTART_BASE_DELAY, Duration::from_millis(250));
}

#[test]
fn retry_state_is_scoped() {
    let mut runtime = WebSurfaceRuntime::new();
    let scope_a = WebSurfaceRuntimeScope::new(ProfileId::new(), ProfileDataMode::Transient);
    let scope_b = WebSurfaceRuntimeScope::new(ProfileId::new(), ProfileDataMode::Transient);
    let now = Instant::now();

    runtime.note_scope_failure(&scope_a, now);
    runtime.note_scope_failure(&scope_a, now);
    runtime.note_scope_failure(&scope_b, now);

    assert_eq!(runtime.retry_state.get(&scope_a).map(|state| state.failure_count), Some(2));
    assert_eq!(runtime.retry_state.get(&scope_b).map(|state| state.failure_count), Some(1));
    assert_eq!(runtime.retry_state.get(&scope_b).map(|state| state.failure_count), Some(1));
}

#[test]
fn successful_frame_resets_scope_retry_state() {
    let mut runtime = WebSurfaceRuntime::new();
    let scope = WebSurfaceRuntimeScope::new(ProfileId::new(), ProfileDataMode::Transient);
    let tab_id = TabId::new();
    let generation = RequestGeneration::new(1);
    let session = session_for_scope(&mut runtime.sessions, &tab_id, scope.clone());
    session.generation = Some(generation);
    session.frame_generation_floor = Some(generation);
    runtime.note_scope_failure(&scope, Instant::now());
    let mut frames = Vec::new();

    runtime.collect_responses(
        &scope,
        vec![WorkerResponse::Frame {
            generation,
            tab_id: tab_id.as_str().to_string(),
            frame: ServoLiveFrame::for_test(1, 1, vec![16, 32, 64, 255]),
        }],
        Instant::now(),
        &mut frames,
    );

    assert!(!runtime.retry_state.contains_key(&scope));
    assert_eq!(frames.len(), 1);
}
