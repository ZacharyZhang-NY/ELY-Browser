use std::{
    collections::{BTreeMap, VecDeque},
    io,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread::JoinHandle,
};

use crate::services::servo_live::{
    ServoLiveClient, ServoLiveEnsureRequest, ServoLiveError, ServoLiveFrame,
    ServoLivePermissionGrant,
};

#[path = "web_surface_worker_dispatch.rs"]
mod dispatch;
use dispatch::{
    can_merge_consecutive_scroll, dispatch_result, forward_permission_consumptions,
    merge_consecutive_scroll, preserve_latest_hover, request_has_ordered_input,
};

/// Blocking transport for one profile-scoped Servo sidecar.
///
/// Production wraps [`ServoLiveClient`]; tests substitute a fake. Every
/// call may block on sidecar IPC, so implementations live on a worker.
pub(super) trait LiveRuntimeClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError>;

    fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError>;

    fn reload(&mut self, tab_id: String) -> Result<(), LiveRuntimeClientError> {
        let _ = tab_id;
        Ok(())
    }

    fn close(&mut self, tab_id: String) -> Result<(), LiveRuntimeClientError>;

    fn take_permission_consumptions(&mut self) -> Vec<ServoLivePermissionGrant> {
        Vec::new()
    }
}

impl LiveRuntimeClient for ServoLiveClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        ServoLiveClient::ensure(self, request).map_err(LiveRuntimeClientError::from)
    }

    fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        ServoLiveClient::poll(self, tab_id).map_err(LiveRuntimeClientError::from)
    }

    fn reload(&mut self, tab_id: String) -> Result<(), LiveRuntimeClientError> {
        ServoLiveClient::reload(self, tab_id).map_err(LiveRuntimeClientError::from)
    }

    fn close(&mut self, tab_id: String) -> Result<(), LiveRuntimeClientError> {
        ServoLiveClient::close(self, tab_id).map_err(LiveRuntimeClientError::from)
    }

    fn take_permission_consumptions(&mut self) -> Vec<ServoLivePermissionGrant> {
        ServoLiveClient::take_permission_consumptions(self)
    }
}

#[derive(Debug)]
pub(super) enum LiveRuntimeClientError {
    RuntimeUnavailable,
    Message(String),
}

impl LiveRuntimeClientError {
    pub(super) fn is_runtime_unavailable(&self) -> bool {
        matches!(self, Self::RuntimeUnavailable)
    }
}

impl std::fmt::Display for LiveRuntimeClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeUnavailable => formatter.write_str("servo sidecar runtime is unavailable"),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl From<String> for LiveRuntimeClientError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<ServoLiveError> for LiveRuntimeClientError {
    fn from(error: ServoLiveError) -> Self {
        if error.is_runtime_unavailable() {
            return Self::RuntimeUnavailable;
        }
        Self::Message(error.to_string())
    }
}

impl From<io::Error> for LiveRuntimeClientError {
    fn from(error: io::Error) -> Self {
        Self::Message(error.to_string())
    }
}

/// Output of a worker request.
pub(super) enum WorkerResponse {
    Frame { generation: RequestGeneration, tab_id: String, frame: ServoLiveFrame },
    Failed { generation: RequestGeneration, tab_id: String, message: String },
    RuntimeUnavailable,
    PermissionSnapshotAccepted(ServoLivePermissionGrant),
    PermissionConsumed(ServoLivePermissionGrant),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RequestGeneration(u64);

impl RequestGeneration {
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(super) const fn value(self) -> u64 {
        self.0
    }
}

enum WorkerRequest {
    Ensure { generation: RequestGeneration, request: Box<ServoLiveEnsureRequest> },
    Poll { generation: RequestGeneration, tab_id: String },
}

impl WorkerRequest {
    fn tab_id(&self) -> &str {
        match self {
            Self::Ensure { request, .. } => request.tab_id.as_str(),
            Self::Poll { tab_id, .. } => tab_id.as_str(),
        }
    }

    fn failure_parts(self) -> (RequestGeneration, String) {
        match self {
            Self::Ensure { generation, request } => (generation, request.tab_id),
            Self::Poll { generation, tab_id } => (generation, tab_id),
        }
    }
}

struct WorkerQueue {
    /// Ordered inputs queue; idle and hover updates coalesce at the tail.
    pending: BTreeMap<String, VecDeque<WorkerRequest>>,
    /// Round-robin tab order, with each pending tab represented once.
    ready_tabs: VecDeque<String>,
    reloads: VecDeque<String>,
    closes: VecDeque<String>,
    in_flight: bool,
    in_flight_tab: Option<String>,
    initialization_failure: Option<String>,
    shutdown: bool,
}

/// Runs one blocking profile client behind a non-blocking fair queue.
pub(super) struct LiveRuntimeWorker {
    queue: Arc<(Mutex<WorkerQueue>, Condvar)>,
    response_tx: mpsc::Sender<WorkerResponse>,
    response_rx: mpsc::Receiver<WorkerResponse>,
    thread: Option<JoinHandle<()>>,
}

impl LiveRuntimeWorker {
    pub(super) fn new(
        client_factory: impl FnOnce() -> Result<Box<dyn LiveRuntimeClient>, String> + Send + 'static,
    ) -> Result<Self, String> {
        let queue = Arc::new((
            Mutex::new(WorkerQueue {
                pending: BTreeMap::new(),
                ready_tabs: VecDeque::new(),
                reloads: VecDeque::new(),
                closes: VecDeque::new(),
                in_flight: false,
                in_flight_tab: None,
                initialization_failure: None,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let (response_tx, response_rx) = mpsc::channel();
        let queue_for_thread = queue.clone();
        let response_for_thread = response_tx.clone();
        let thread = std::thread::Builder::new()
            .name("ely-servo-runtime".to_string())
            .spawn(move || match client_factory() {
                Ok(client) => run_worker(client, queue_for_thread, response_for_thread),
                Err(error) => {
                    fail_worker_initialization(queue_for_thread, &response_for_thread, error);
                }
            })
            .map_err(|error| format!("failed to spawn servo live worker thread: {error}"))?;
        Ok(Self { queue, response_tx, response_rx, thread: Some(thread) })
    }

    pub(super) fn submit_ensure(
        &self,
        generation: RequestGeneration,
        request: ServoLiveEnsureRequest,
    ) {
        let tab_id = request.tab_id.clone();
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if q.shutdown {
            return;
        }
        if let Some(message) = q.initialization_failure.clone() {
            drop(q);
            let _ = self.response_tx.send(WorkerResponse::Failed { generation, tab_id, message });
            return;
        }
        let mut request = WorkerRequest::Ensure { generation, request: Box::new(request) };
        if let Some(pending) = q.pending.get_mut(&tab_id) {
            if let Some(tail) = pending.back_mut()
                && (can_merge_consecutive_scroll(&request, tail)
                    || matches!(tail, WorkerRequest::Poll { .. })
                    || (!request_has_ordered_input(&request) && !request_has_ordered_input(tail)))
            {
                merge_consecutive_scroll(&mut request, tail);
                preserve_latest_hover(&mut request, tail);
                *tail = request;
            } else {
                pending.push_back(request);
            }
        } else {
            q.pending.insert(tab_id.clone(), VecDeque::from([request]));
            q.ready_tabs.push_back(tab_id);
        }
        cvar.notify_one();
    }
    pub(super) fn submit_poll(&self, generation: RequestGeneration, tab_id: String) -> bool {
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if q.shutdown {
            return false;
        }
        if let Some(message) = q.initialization_failure.clone() {
            drop(q);
            let _ = self.response_tx.send(WorkerResponse::Failed { generation, tab_id, message });
            return true;
        }
        // A pending Ensure already produces the latest frame after its
        // run; don't downgrade it to a Poll. Only insert if nothing is
        // queued.
        let inserted = if q.pending.contains_key(&tab_id)
            || q.in_flight_tab.as_deref() == Some(tab_id.as_str())
        {
            false
        } else {
            q.pending.insert(
                tab_id.clone(),
                VecDeque::from([WorkerRequest::Poll { generation, tab_id: tab_id.clone() }]),
            );
            q.ready_tabs.push_back(tab_id);
            true
        };
        cvar.notify_one();
        inserted
    }

    pub(super) fn submit_reload(&self, tab_id: String) {
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if q.shutdown || q.initialization_failure.is_some() {
            return;
        }
        if !q.reloads.contains(&tab_id) {
            q.reloads.push_back(tab_id);
        }
        cvar.notify_one();
    }

    pub(super) fn submit_close(&self, tab_id: String) {
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if q.shutdown {
            return;
        }
        if q.initialization_failure.is_some() {
            return;
        }
        q.pending.remove(&tab_id);
        q.ready_tabs.retain(|ready_tab_id| ready_tab_id != &tab_id);
        q.reloads.retain(|reload_tab_id| reload_tab_id != &tab_id);
        q.closes.push_back(tab_id);
        cvar.notify_one();
    }

    pub(super) fn drain_responses(&self) -> Vec<WorkerResponse> {
        let mut out = Vec::new();
        while let Ok(response) = self.response_rx.try_recv() {
            out.push(response);
        }
        out
    }

    /// Test-only barrier for all submitted work.
    #[cfg(test)]
    pub(super) fn wait_until_idle(&self) {
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        while !q.pending.is_empty() || !q.reloads.is_empty() || !q.closes.is_empty() || q.in_flight
        {
            q = match cvar.wait(q) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

impl Drop for LiveRuntimeWorker {
    fn drop(&mut self) {
        {
            let (lock, cvar) = &*self.queue;
            let mut q = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.shutdown = true;
            cvar.notify_all();
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn fail_worker_initialization(
    queue: Arc<(Mutex<WorkerQueue>, Condvar)>,
    response_tx: &mpsc::Sender<WorkerResponse>,
    message: String,
) {
    let (lock, cvar) = &*queue;
    let mut q = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    q.initialization_failure = Some(message.clone());
    q.ready_tabs.clear();
    q.reloads.clear();
    q.closes.clear();
    q.in_flight = false;
    q.in_flight_tab = None;
    let pending = std::mem::take(&mut q.pending);
    for request in pending.into_values().flatten() {
        let (generation, tab_id) = request.failure_parts();
        let _ = response_tx.send(WorkerResponse::Failed {
            generation,
            tab_id,
            message: message.clone(),
        });
    }
    let _ = response_tx.send(WorkerResponse::RuntimeUnavailable);
    cvar.notify_all();
}

fn run_worker(
    mut client: Box<dyn LiveRuntimeClient>,
    queue: Arc<(Mutex<WorkerQueue>, Condvar)>,
    response_tx: mpsc::Sender<WorkerResponse>,
) {
    let (lock, cvar) = &*queue;
    let mut last_dispatched_tab = None;
    loop {
        let work = {
            let mut q = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.in_flight = false;
            q.in_flight_tab = None;
            cvar.notify_all();
            while q.pending.is_empty() && q.reloads.is_empty() && q.closes.is_empty() && !q.shutdown
            {
                q = match cvar.wait(q) {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }
            if q.shutdown {
                return;
            }
            let next = if let Some(close_id) = q.closes.pop_front() {
                Work::Close(close_id)
            } else if let Some(reload_id) = q.reloads.pop_front() {
                Work::Reload(reload_id)
            } else {
                if q.ready_tabs.len() > 1
                    && q.ready_tabs.front() == last_dispatched_tab.as_ref()
                    && let Some(last_tab) = q.ready_tabs.pop_front()
                {
                    q.ready_tabs.push_back(last_tab);
                }
                let tab_id = match q.ready_tabs.pop_front() {
                    Some(tab_id) => tab_id,
                    None => continue,
                };
                let (request, has_more) = match q.pending.get_mut(&tab_id) {
                    Some(pending) => match pending.pop_front() {
                        Some(request) => (request, !pending.is_empty()),
                        None => continue,
                    },
                    None => continue,
                };
                if has_more {
                    q.ready_tabs.push_back(tab_id.clone());
                } else {
                    q.pending.remove(&tab_id);
                }
                last_dispatched_tab = Some(tab_id);
                Work::Request(request)
            };
            q.in_flight = true;
            q.in_flight_tab = match &next {
                Work::Close(_) | Work::Reload(_) => None,
                Work::Request(request) => Some(request.tab_id().to_string()),
            };
            next
        };

        let exit_after_dispatch = match work {
            Work::Close(tab_id) => {
                let _ = client.close(tab_id);
                forward_permission_consumptions(&mut *client, &response_tx);
                false
            }
            Work::Reload(tab_id) => {
                let _ = client.reload(tab_id);
                forward_permission_consumptions(&mut *client, &response_tx);
                false
            }
            Work::Request(WorkerRequest::Ensure { generation, mut request }) => {
                let tab_id = request.tab_id.clone();
                let allow_once_grants = std::mem::take(&mut request.allow_once_grants);
                let result = client.ensure(*request);
                if result.is_ok() {
                    for grant in allow_once_grants {
                        let _ = response_tx.send(WorkerResponse::PermissionSnapshotAccepted(grant));
                    }
                }
                forward_permission_consumptions(&mut *client, &response_tx);
                dispatch_result(&response_tx, generation, tab_id, result)
            }
            Work::Request(WorkerRequest::Poll { generation, tab_id }) => {
                let request_tab_id = tab_id.clone();
                let result = client.poll(tab_id);
                forward_permission_consumptions(&mut *client, &response_tx);
                dispatch_result(&response_tx, generation, request_tab_id, result)
            }
        };

        if exit_after_dispatch {
            let mut q = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.in_flight = false;
            q.in_flight_tab = None;
            cvar.notify_all();
            return;
        }
    }
}

enum Work {
    Close(String),
    Reload(String),
    Request(WorkerRequest),
}

#[cfg(test)]
#[path = "web_surface_worker_tests.rs"]
mod tests;
