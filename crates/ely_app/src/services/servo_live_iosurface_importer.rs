#![cfg(target_os = "macos")]

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::services::{
    iosurface_mach::IOSurfaceMachReceiver,
    iosurface_metal::{ImportedPixelBuffer, import_pixel_buffer_from_mach_port},
};

use super::wire::LiveSurfaceHandle;

const RECEIVE_TIMEOUT: Duration = Duration::from_secs(1);
const IMPORT_QUEUE_CAPACITY: usize = 16;

pub(super) struct IOSurfaceImportWorker {
    request_tx: Option<mpsc::SyncSender<LiveSurfaceHandle>>,
    result_rx: mpsc::Receiver<IOSurfaceImportResult>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl IOSurfaceImportWorker {
    pub(super) fn new(receiver: IOSurfaceMachReceiver) -> Result<Self, io::Error> {
        let (request_tx, request_rx) = mpsc::sync_channel(IMPORT_QUEUE_CAPACITY);
        let (result_tx, result_rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let thread = thread::Builder::new()
            .name("ely-iosurface-import".to_string())
            .spawn(move || run_import_worker(receiver, request_rx, result_tx, worker_shutdown))?;
        Ok(Self { request_tx: Some(request_tx), result_rx, shutdown, thread: Some(thread) })
    }

    pub(super) fn submit(&self, handle: LiveSurfaceHandle) -> Result<(), IOSurfaceImportFailure> {
        let Some(request_tx) = self.request_tx.as_ref() else {
            return Err(IOSurfaceImportFailure::worker_stopped(handle));
        };
        request_tx.send(handle).map_err(|error| IOSurfaceImportFailure::worker_stopped(error.0))
    }

    pub(super) fn drain(&self) -> Vec<IOSurfaceImportResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            results.push(result);
        }
        results
    }
}

impl Drop for IOSurfaceImportWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.request_tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) enum IOSurfaceImportResult {
    Imported(ImportedIOSurface),
    Failed(IOSurfaceImportFailure),
}

pub(super) struct ImportedIOSurface {
    pub(super) surface_id: u64,
    pub(super) imported: ImportedPixelBuffer,
}

// SAFETY: `ImportedPixelBuffer` owns retained IOSurface and CVPixelBuffer
// handles plus a use-count guard. CoreFoundation retain/release and IOSurface
// use-count operations are thread-safe while this value crosses the channel.
#[expect(unsafe_code)]
unsafe impl Send for ImportedIOSurface {}

pub(super) struct IOSurfaceImportFailure {
    pub(super) surface_id: u64,
    pub(super) mach_port_name: u32,
    pub(super) message: String,
}

impl IOSurfaceImportFailure {
    fn worker_stopped(handle: LiveSurfaceHandle) -> Self {
        Self::queue_rejected(handle, "IOSurface import worker stopped")
    }

    fn queue_rejected(handle: LiveSurfaceHandle, message: &str) -> Self {
        Self {
            surface_id: handle.surface_id,
            mach_port_name: handle.mach_port_name,
            message: message.to_string(),
        }
    }
}

fn run_import_worker(
    mut receiver: IOSurfaceMachReceiver,
    request_rx: mpsc::Receiver<LiveSurfaceHandle>,
    result_tx: mpsc::Sender<IOSurfaceImportResult>,
    shutdown: Arc<AtomicBool>,
) {
    run_import_requests(request_rx, result_tx, shutdown, |handle| {
        import_surface_handle(&mut receiver, handle)
    });
}

fn run_import_requests(
    request_rx: mpsc::Receiver<LiveSurfaceHandle>,
    result_tx: mpsc::Sender<IOSurfaceImportResult>,
    shutdown: Arc<AtomicBool>,
    mut import: impl FnMut(LiveSurfaceHandle) -> IOSurfaceImportResult,
) {
    while let Ok(handle) = request_rx.recv() {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let result = import(handle);
        if shutdown.load(Ordering::Acquire) || result_tx.send(result).is_err() {
            return;
        }
    }
}

fn import_surface_handle(
    receiver: &mut IOSurfaceMachReceiver,
    handle: LiveSurfaceHandle,
) -> IOSurfaceImportResult {
    let mach_port_name = match receiver.receive_port_for_surface(handle.surface_id, RECEIVE_TIMEOUT)
    {
        Ok(mach_port_name) => mach_port_name,
        Err(error) => {
            return IOSurfaceImportResult::Failed(IOSurfaceImportFailure {
                surface_id: handle.surface_id,
                mach_port_name: handle.mach_port_name,
                message: error.to_string(),
            });
        }
    };

    match import_pixel_buffer_from_mach_port(mach_port_name) {
        Ok(imported)
            if imported_surface_matches(
                &handle,
                imported.system_surface_id,
                imported.backing.pixel_buffer().get_width(),
                imported.backing.pixel_buffer().get_height(),
            ) =>
        {
            IOSurfaceImportResult::Imported(ImportedIOSurface {
                surface_id: handle.surface_id,
                imported,
            })
        }
        Ok(imported) => IOSurfaceImportResult::Failed(IOSurfaceImportFailure {
            surface_id: handle.surface_id,
            mach_port_name,
            message: format!(
                "IOSurface id/size {:#x} {}x{} did not match handle {:#x} {}x{}",
                imported.system_surface_id,
                imported.backing.pixel_buffer().get_width(),
                imported.backing.pixel_buffer().get_height(),
                handle.surface_id,
                handle.width,
                handle.height,
            ),
        }),
        Err(error) => IOSurfaceImportResult::Failed(IOSurfaceImportFailure {
            surface_id: handle.surface_id,
            mach_port_name,
            message: error.to_string(),
        }),
    }
}

fn imported_surface_matches(
    handle: &LiveSurfaceHandle,
    system_surface_id: u64,
    width: usize,
    height: usize,
) -> bool {
    system_surface_id == handle.surface_id
        && width == handle.width as usize
        && height == handle.height as usize
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, sync::mpsc};

    use super::*;

    #[test]
    fn shutdown_stops_after_current_import() {
        let (request_tx, request_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        for surface_id in 1..=3 {
            assert!(request_tx.send(handle(surface_id)).is_ok());
        }
        drop(request_tx);
        let processed = Cell::new(0);
        let import_shutdown = shutdown.clone();

        run_import_requests(request_rx, result_tx, shutdown, |handle| {
            processed.set(processed.get() + 1);
            import_shutdown.store(true, Ordering::Release);
            IOSurfaceImportResult::Failed(IOSurfaceImportFailure::worker_stopped(handle))
        });

        assert_eq!(processed.get(), 1);
        assert!(result_rx.try_recv().is_err());
    }

    #[test]
    fn imported_surface_requires_matching_system_id_and_dimensions() {
        assert!(imported_surface_matches(&handle(7), 7, 64, 48));
        assert!(!imported_surface_matches(&handle(8), 7, 64, 48));
        let mut wrong_size = handle(7);
        wrong_size.width = 96;
        assert!(!imported_surface_matches(&wrong_size, 7, 64, 48));
    }

    #[test]
    fn submit_backpressures_surface_burst_at_queue_capacity() {
        let (request_tx, request_rx) = mpsc::sync_channel(IMPORT_QUEUE_CAPACITY);
        let (_result_tx, result_rx) = mpsc::channel();
        let worker = IOSurfaceImportWorker {
            request_tx: Some(request_tx),
            result_rx,
            shutdown: Arc::new(AtomicBool::new(false)),
            thread: None,
        };
        for surface_id in 0..IMPORT_QUEUE_CAPACITY as u64 {
            assert!(worker.submit(handle(surface_id)).is_ok());
        }
        let (release_tx, release_rx) = mpsc::channel();
        let consumer = std::thread::spawn(move || {
            let received = request_rx.recv();
            let _ = release_rx.recv();
            received
        });

        assert!(worker.submit(handle(99)).is_ok());
        assert!(release_tx.send(()).is_ok());
        assert!(consumer.join().is_ok());
    }

    fn handle(surface_id: u64) -> LiveSurfaceHandle {
        LiveSurfaceHandle { mach_port_name: surface_id as u32, surface_id, width: 64, height: 48 }
    }
}
