use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread::JoinHandle,
};

use crate::services::servo_live::{
    ServoLiveClient, ServoLiveEnsureRequest, ServoLiveError, ServoLiveFrame,
};

/// Blocking surface for the embedded Servo runtime.
///
/// Production wraps [`ServoLiveClient`] directly; tests substitute a
/// fake. The contract: every call is blocking and may run for tens of
/// milliseconds. Implementations live on the worker thread.
pub(super) trait LiveRuntimeClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError>;

    fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError>;

    fn close(&mut self, tab_id: String) -> Result<(), LiveRuntimeClientError>;
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

    fn close(&mut self, tab_id: String) -> Result<(), LiveRuntimeClientError> {
        ServoLiveClient::close(self, tab_id).map_err(LiveRuntimeClientError::from)
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
            Self::RuntimeUnavailable => formatter.write_str("servo live runtime is unavailable"),
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
    Frame { tab_id: String, frame: ServoLiveFrame },
    Failed { tab_id: String, message: String },
    RuntimeUnavailable,
}

enum WorkerRequest {
    Ensure(ServoLiveEnsureRequest),
    Poll { tab_id: String },
}

struct WorkerQueue {
    /// Latest request per tab. A new submission for a tab replaces any
    /// earlier in-flight-but-not-yet-started request, so a flurry of
    /// scrolls never piles up — the worker always processes the most
    /// recent frame's worth of inputs.
    pending: BTreeMap<String, WorkerRequest>,
    /// Close orders. Sent after pending is cleared for that tab so the
    /// worker never closes a tab that still has live frames in flight.
    closes: Vec<String>,
    /// True while the worker is processing a request. `wait_until_idle`
    /// uses this alongside the queue emptiness to know when all
    /// previously-submitted work has actually run.
    in_flight: bool,
    shutdown: bool,
}

/// Owns a [`LiveRuntimeClient`] on a dedicated OS thread and exposes
/// a non-blocking API: submit ensure/poll/close, then drain responses.
///
/// The UI thread never blocks on Servo. Submissions push into a
/// coalescing queue (latest request per tab wins). The worker thread
/// drains the queue, runs the blocking calls, and emits responses on a
/// `std::sync::mpsc` channel that the UI thread reads with `try_recv`.
pub(super) struct LiveRuntimeWorker {
    queue: Arc<(Mutex<WorkerQueue>, Condvar)>,
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
                closes: Vec::new(),
                in_flight: false,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let (response_tx, response_rx) = mpsc::channel();
        let (init_tx, init_rx) = mpsc::channel();
        let queue_for_thread = queue.clone();
        let thread = std::thread::Builder::new()
            .name("ely-servo-runtime".to_string())
            .spawn(move || {
                let client = match client_factory() {
                    Ok(client) => {
                        let _ = init_tx.send(Ok(()));
                        client
                    }
                    Err(error) => {
                        let _ = init_tx.send(Err(error));
                        return;
                    }
                };
                run_worker(client, queue_for_thread, response_tx);
            })
            .map_err(|error| format!("failed to spawn servo live worker thread: {error}"))?;
        match init_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(error);
            }
            Err(error) => {
                let _ = thread.join();
                return Err(format!("servo live worker initialization failed: {error}"));
            }
        }
        Ok(Self { queue, response_rx, thread: Some(thread) })
    }

    pub(super) fn submit_ensure(&self, request: ServoLiveEnsureRequest) {
        let tab_id = request.tab_id.clone();
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if q.shutdown {
            return;
        }
        q.pending.insert(tab_id, WorkerRequest::Ensure(request));
        cvar.notify_one();
    }

    pub(super) fn submit_poll(&self, tab_id: String) -> bool {
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if q.shutdown {
            return false;
        }
        // A pending Ensure already produces the latest frame after its
        // run; don't downgrade it to a Poll. Only insert if nothing is
        // queued.
        let inserted = match q.pending.entry(tab_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(WorkerRequest::Poll { tab_id });
                true
            }
            std::collections::btree_map::Entry::Occupied(_) => false,
        };
        cvar.notify_one();
        inserted
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
        q.pending.remove(&tab_id);
        q.closes.push(tab_id);
        cvar.notify_one();
    }

    pub(super) fn drain_responses(&self) -> Vec<WorkerResponse> {
        let mut out = Vec::new();
        while let Ok(response) = self.response_rx.try_recv() {
            out.push(response);
        }
        out
    }

    /// Test-only barrier. Blocks the caller until the worker has
    /// drained everything currently submitted. Production code never
    /// waits — the whole point of the worker is that the UI thread
    /// progresses without IPC latency.
    #[cfg(test)]
    pub(super) fn wait_until_idle(&self) {
        let (lock, cvar) = &*self.queue;
        let mut q = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        while !q.pending.is_empty() || !q.closes.is_empty() || q.in_flight {
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
            if let Ok(mut q) = lock.lock() {
                q.shutdown = true;
                cvar.notify_all();
            }
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn run_worker(
    mut client: Box<dyn LiveRuntimeClient>,
    queue: Arc<(Mutex<WorkerQueue>, Condvar)>,
    response_tx: mpsc::Sender<WorkerResponse>,
) {
    let (lock, cvar) = &*queue;
    loop {
        let work = {
            let mut q = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.in_flight = false;
            cvar.notify_all();
            while q.pending.is_empty() && q.closes.is_empty() && !q.shutdown {
                q = match cvar.wait(q) {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }
            if q.shutdown {
                return;
            }
            let next = if let Some(close_id) = q.closes.pop() {
                Work::Close(close_id)
            } else {
                let key = match q.pending.keys().next().cloned() {
                    Some(key) => key,
                    None => continue,
                };
                let Some(request) = q.pending.remove(&key) else {
                    continue;
                };
                Work::Request(request)
            };
            q.in_flight = true;
            next
        };

        let exit_after_dispatch = match work {
            Work::Close(tab_id) => {
                let _ = client.close(tab_id);
                false
            }
            Work::Request(WorkerRequest::Ensure(request)) => {
                let tab_id = request.tab_id.clone();
                dispatch_result(&response_tx, tab_id, client.ensure(request))
            }
            Work::Request(WorkerRequest::Poll { tab_id }) => {
                let request_tab_id = tab_id.clone();
                dispatch_result(&response_tx, request_tab_id, client.poll(tab_id))
            }
        };

        if exit_after_dispatch {
            // Release the in-flight flag and wake any flush waiter
            // before exiting so wait_until_idle doesn't block forever
            // on a thread that has already returned.
            let mut q = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.in_flight = false;
            cvar.notify_all();
            return;
        }
    }
}

enum Work {
    Close(String),
    Request(WorkerRequest),
}

/// Forward a single client result to the response channel. Returns
/// `true` when the worker should exit.
fn dispatch_result(
    response_tx: &mpsc::Sender<WorkerResponse>,
    tab_id: String,
    result: Result<Option<ServoLiveFrame>, LiveRuntimeClientError>,
) -> bool {
    match result {
        Ok(Some(frame)) => {
            let _ = response_tx.send(WorkerResponse::Frame { tab_id, frame });
            false
        }
        Ok(None) => false,
        Err(error) => {
            let unavailable = error.is_runtime_unavailable();
            let message = error.to_string();
            let _ = response_tx.send(WorkerResponse::Failed { tab_id, message });
            if unavailable {
                let _ = response_tx.send(WorkerResponse::RuntimeUnavailable);
                return true;
            }
            false
        }
    }
}
