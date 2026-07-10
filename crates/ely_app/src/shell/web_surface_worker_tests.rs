use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use crate::services::servo_live::{
    ServoLiveEnsureRequest, ServoLiveFrame, ServoLivePermissionGrant, ServoLiveSitePermission,
};
use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature};

use super::{
    LiveRuntimeClient, LiveRuntimeClientError, LiveRuntimeWorker, RequestGeneration, WorkerResponse,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum RecordedInput {
    Idle(String),
    Scroll(String),
    Click(String),
    Text(String),
    Hover(String),
    Permission(String),
}

struct SlowRecordingClient {
    calls: Arc<Mutex<Vec<RecordedInput>>>,
    first_started_tx: Option<mpsc::Sender<()>>,
    release_first_rx: mpsc::Receiver<()>,
    return_frame: bool,
}

struct GenerationClient;

struct ConsumptionClient {
    pending: Vec<ServoLivePermissionGrant>,
}

impl LiveRuntimeClient for ConsumptionClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Err(LiveRuntimeClientError::Message("ensure failed".to_string()))
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }

    fn take_permission_consumptions(&mut self) -> Vec<ServoLivePermissionGrant> {
        std::mem::take(&mut self.pending)
    }
}

impl LiveRuntimeClient for GenerationClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(Some(ServoLiveFrame::for_test(1, 1, vec![16, 32, 64, 255])))
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Err(LiveRuntimeClientError::Message("poll failed".to_string()))
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

impl LiveRuntimeClient for SlowRecordingClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        let input = recorded_input(&request);
        self.calls.lock().map_err(|_| "call recorder lock was poisoned".to_string())?.push(input);
        if let Some(first_started_tx) = self.first_started_tx.take() {
            first_started_tx.send(()).map_err(|error| error.to_string())?;
            self.release_first_rx.recv().map_err(|error| error.to_string())?;
        }
        Ok(self.return_frame.then(|| ServoLiveFrame::for_test(1, 1, vec![16, 32, 64, 255])))
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

#[test]
fn queued_edge_inputs_for_one_tab_are_preserved() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let client_calls = calls.clone();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(SlowRecordingClient {
            calls: client_calls,
            first_started_tx: Some(first_started_tx),
            release_first_rx,
            return_frame: false,
        }))
    })?;

    worker.submit_ensure(
        RequestGeneration::new(1),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    first_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    for (generation, input) in [
        RecordedInput::Scroll("tab-a".to_string()),
        RecordedInput::Click("tab-a".to_string()),
        RecordedInput::Text("tab-a".to_string()),
        RecordedInput::Hover("tab-a".to_string()),
    ]
    .into_iter()
    .enumerate()
    {
        worker.submit_ensure(
            RequestGeneration::new(generation as u64 + 2),
            ensure_request("tab-a", input),
        );
    }
    release_first_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();

    assert_eq!(
        *calls.lock().map_err(|_| "call recorder lock was poisoned".to_string())?,
        vec![
            RecordedInput::Idle("tab-a".to_string()),
            RecordedInput::Scroll("tab-a".to_string()),
            RecordedInput::Click("tab-a".to_string()),
            RecordedInput::Text("tab-a".to_string()),
            RecordedInput::Hover("tab-a".to_string()),
        ]
    );
    Ok(())
}

#[test]
fn allow_once_transfer_is_preserved_ahead_of_idle_updates() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let client_calls = calls.clone();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(SlowRecordingClient {
            calls: client_calls,
            first_started_tx: Some(first_started_tx),
            release_first_rx,
            return_frame: false,
        }))
    })?;

    worker.submit_ensure(
        RequestGeneration::new(1),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    first_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    let mut transfer = ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string()));
    transfer.site_permissions.push(ServoLiveSitePermission::new(
        "https://example.com",
        "camera",
        "allow-once",
        7,
    ));
    worker.submit_ensure(RequestGeneration::new(2), transfer);
    worker.submit_ensure(
        RequestGeneration::new(3),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    release_first_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();

    assert_eq!(
        *calls.lock().map_err(|_| "call recorder lock was poisoned".to_string())?,
        vec![
            RecordedInput::Idle("tab-a".to_string()),
            RecordedInput::Permission("tab-a".to_string()),
            RecordedInput::Idle("tab-a".to_string()),
        ],
    );
    Ok(())
}

#[test]
fn queued_tabs_are_dispatched_round_robin() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let client_calls = calls.clone();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(SlowRecordingClient {
            calls: client_calls,
            first_started_tx: Some(first_started_tx),
            release_first_rx,
            return_frame: false,
        }))
    })?;

    worker.submit_ensure(
        RequestGeneration::new(1),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    first_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    worker.submit_ensure(
        RequestGeneration::new(2),
        ensure_request("tab-a", RecordedInput::Click("tab-a".to_string())),
    );
    worker.submit_ensure(
        RequestGeneration::new(3),
        ensure_request("tab-b", RecordedInput::Click("tab-b".to_string())),
    );
    worker.submit_ensure(
        RequestGeneration::new(4),
        ensure_request("tab-a", RecordedInput::Text("tab-a".to_string())),
    );
    release_first_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();

    assert_eq!(
        *calls.lock().map_err(|_| "call recorder lock was poisoned".to_string())?,
        vec![
            RecordedInput::Idle("tab-a".to_string()),
            RecordedInput::Click("tab-b".to_string()),
            RecordedInput::Click("tab-a".to_string()),
            RecordedInput::Text("tab-a".to_string()),
        ]
    );
    Ok(())
}

#[test]
fn responses_keep_their_request_generation() -> Result<(), String> {
    let worker = LiveRuntimeWorker::new(|| Ok(Box::new(GenerationClient)))?;
    let ensure_generation = RequestGeneration::new(41);
    let poll_generation = RequestGeneration::new(42);

    worker.submit_ensure(
        ensure_generation,
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    worker.wait_until_idle();
    assert!(matches!(
        worker.drain_responses().as_slice(),
        [WorkerResponse::Frame { generation, tab_id, .. }]
            if *generation == ensure_generation && tab_id == "tab-a"
    ));

    assert!(worker.submit_poll(poll_generation, "tab-a".to_string()));
    worker.wait_until_idle();
    assert!(matches!(
        worker.drain_responses().as_slice(),
        [WorkerResponse::Failed { generation, tab_id, message }]
            if *generation == poll_generation && tab_id == "tab-a" && message == "poll failed"
    ));
    Ok(())
}

#[test]
fn permission_consumption_is_forwarded_after_ensure_error() -> Result<(), String> {
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com").map_err(|error| error.to_string())?;
    let grant = ServoLivePermissionGrant::new(profile_id, origin, SitePermissionFeature::Camera, 7);
    let expected = grant.clone();
    let consumed = grant.clone();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(ConsumptionClient { pending: vec![consumed] }))
    })?;

    let mut request = ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string()));
    request.allow_once_grants.push(grant);
    worker.submit_ensure(RequestGeneration::new(1), request);
    worker.wait_until_idle();

    assert!(matches!(
        worker.drain_responses().as_slice(),
        [
            WorkerResponse::PermissionConsumed(consumed),
            WorkerResponse::Failed { message, .. },
        ] if consumed == &expected && message == "ensure failed"
    ));
    Ok(())
}

#[test]
fn hover_updates_coalesce_to_latest_state() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let client_calls = calls.clone();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(SlowRecordingClient {
            calls: client_calls,
            first_started_tx: Some(first_started_tx),
            release_first_rx,
            return_frame: false,
        }))
    })?;

    worker.submit_ensure(
        RequestGeneration::new(1),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    first_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    for generation in 2..=101 {
        worker.submit_ensure(
            RequestGeneration::new(generation),
            ensure_request("tab-a", RecordedInput::Hover("tab-a".to_string())),
        );
    }
    release_first_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();

    assert_eq!(
        *calls.lock().map_err(|_| "call recorder lock was poisoned".to_string())?,
        vec![RecordedInput::Idle("tab-a".to_string()), RecordedInput::Hover("tab-a".to_string()),]
    );
    Ok(())
}

#[test]
fn poll_is_rejected_while_same_tab_ensure_is_in_flight() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(SlowRecordingClient {
            calls,
            first_started_tx: Some(first_started_tx),
            release_first_rx,
            return_frame: true,
        }))
    })?;
    let ensure_generation = RequestGeneration::new(1);

    worker.submit_ensure(
        ensure_generation,
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    first_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    assert!(!worker.submit_poll(RequestGeneration::new(2), "tab-a".to_string()));
    release_first_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();

    assert!(matches!(
        worker.drain_responses().as_slice(),
        [WorkerResponse::Frame { generation, .. }] if *generation == ensure_generation
    ));
    Ok(())
}

#[test]
fn worker_creation_does_not_wait_for_client_factory() -> Result<(), String> {
    let (factory_started_tx, factory_started_rx) = mpsc::channel();
    let (release_factory_tx, release_factory_rx) = mpsc::channel();
    let worker = LiveRuntimeWorker::new(move || {
        factory_started_tx.send(()).map_err(|error| error.to_string())?;
        release_factory_rx.recv().map_err(|error| error.to_string())?;
        Ok(Box::new(GenerationClient))
    })?;

    worker.submit_ensure(
        RequestGeneration::new(1),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    factory_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    release_factory_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();
    assert!(matches!(worker.drain_responses().as_slice(), [WorkerResponse::Frame { .. }]));
    Ok(())
}

#[test]
fn factory_failure_rejects_queued_request() -> Result<(), String> {
    let worker = LiveRuntimeWorker::new(|| -> Result<Box<dyn LiveRuntimeClient>, String> {
        Err("injected factory failure".to_string())
    })?;
    worker.submit_ensure(
        RequestGeneration::new(1),
        ensure_request("tab-a", RecordedInput::Idle("tab-a".to_string())),
    );
    worker.wait_until_idle();
    let responses = worker.drain_responses();

    assert!(responses.iter().any(|response| matches!(response, WorkerResponse::Failed { message, .. } if message == "injected factory failure")));
    assert!(
        responses.iter().any(|response| matches!(response, WorkerResponse::RuntimeUnavailable))
    );
    Ok(())
}

#[test]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "this test deliberately poisons the worker queue mutex"
)]
fn poisoned_queue_still_shuts_down_worker() -> Result<(), String> {
    let worker = LiveRuntimeWorker::new(|| Ok(Box::new(GenerationClient)))?;
    let queue = worker.queue.clone();
    let poison_result = std::thread::spawn(move || {
        let (lock, _) = &*queue;
        let _guard = lock.lock().expect("queue lock should begin healthy");
        panic!("injected queue poison");
    })
    .join();

    assert!(poison_result.is_err());
    drop(worker);
    Ok(())
}

fn recorded_input(request: &ServoLiveEnsureRequest) -> RecordedInput {
    let tab_id = request.tab_id.clone();
    if request.site_permissions.iter().any(|permission| permission.state == "allow-once") {
        RecordedInput::Permission(tab_id)
    } else if request.scroll_delta_x != 0 || request.scroll_delta_y != 0 {
        RecordedInput::Scroll(tab_id)
    } else if request.click_x.is_some() {
        RecordedInput::Click(tab_id)
    } else if request.typed_text.is_some() {
        RecordedInput::Text(tab_id)
    } else if request.hover_x.is_some() {
        RecordedInput::Hover(tab_id)
    } else {
        RecordedInput::Idle(tab_id)
    }
}

fn ensure_request(tab_id: &str, input: RecordedInput) -> ServoLiveEnsureRequest {
    let scroll = matches!(input, RecordedInput::Scroll(_));
    let click = matches!(input, RecordedInput::Click(_));
    let text = matches!(input, RecordedInput::Text(_));
    let hover = matches!(input, RecordedInput::Hover(_));
    ServoLiveEnsureRequest {
        tab_id: tab_id.to_string(),
        profile_id: "profile".to_string(),
        url: "https://example.com/".to_string(),
        width: 640,
        height: 480,
        page_zoom_percent: 100,
        device_pixel_ratio: 1.0,
        scroll_delta_x: i32::from(scroll),
        scroll_delta_y: i32::from(scroll),
        scroll_point_x: scroll.then_some(1),
        scroll_point_y: scroll.then_some(1),
        click_x: click.then_some(1),
        click_y: click.then_some(1),
        hover_x: hover.then_some(1),
        hover_y: hover.then_some(1),
        typed_text: text.then(|| "text".to_string()),
        site_permission_generation: 1,
        site_permissions: Vec::new(),
        allow_once_grants: Vec::new(),
    }
}

#[path = "web_surface_worker_scroll_tests.rs"]
mod scroll_tests;
