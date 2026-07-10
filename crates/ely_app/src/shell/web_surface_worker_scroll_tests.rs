use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use crate::services::servo_live::{ServoLiveEnsureRequest, ServoLiveFrame};

use super::super::{
    LiveRuntimeClient, LiveRuntimeClientError, LiveRuntimeWorker, RequestGeneration,
};

struct BlockingScrollClient {
    deltas: Arc<Mutex<Vec<(i32, i32)>>>,
    first_started_tx: Option<mpsc::Sender<()>>,
    release_first_rx: mpsc::Receiver<()>,
}

impl LiveRuntimeClient for BlockingScrollClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        self.deltas
            .lock()
            .map_err(|_| "scroll recorder lock was poisoned".to_string())?
            .push((request.scroll_delta_x, request.scroll_delta_y));
        if let Some(first_started_tx) = self.first_started_tx.take() {
            first_started_tx.send(()).map_err(|error| error.to_string())?;
            self.release_first_rx.recv().map_err(|error| error.to_string())?;
        }
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

#[test]
fn consecutive_scroll_updates_coalesce_without_losing_distance() -> Result<(), String> {
    let deltas = Arc::new(Mutex::new(Vec::new()));
    let client_deltas = deltas.clone();
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let worker = LiveRuntimeWorker::new(move || {
        Ok(Box::new(BlockingScrollClient {
            deltas: client_deltas,
            first_started_tx: Some(first_started_tx),
            release_first_rx,
        }))
    })?;

    worker.submit_ensure(RequestGeneration::new(1), ensure_request(0));
    first_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    for generation in 2..=101 {
        worker.submit_ensure(RequestGeneration::new(generation), ensure_request(1));
    }
    release_first_tx.send(()).map_err(|error| error.to_string())?;
    worker.wait_until_idle();

    assert_eq!(
        *deltas.lock().map_err(|_| "scroll recorder lock was poisoned".to_string())?,
        vec![(0, 0), (100, 100)]
    );
    Ok(())
}

fn ensure_request(scroll_delta: i32) -> ServoLiveEnsureRequest {
    ServoLiveEnsureRequest {
        tab_id: "tab-a".to_string(),
        profile_id: "profile".to_string(),
        url: "https://example.com/".to_string(),
        width: 640,
        height: 480,
        page_zoom_percent: 100,
        device_pixel_ratio: 1.0,
        scroll_delta_x: scroll_delta,
        scroll_delta_y: scroll_delta,
        scroll_point_x: (scroll_delta != 0).then_some(1),
        scroll_point_y: (scroll_delta != 0).then_some(1),
        click_x: None,
        click_y: None,
        hover_x: None,
        hover_y: None,
        typed_text: None,
        site_permission_generation: 1,
        site_permissions: Vec::new(),
        allow_once_grants: Vec::new(),
    }
}
