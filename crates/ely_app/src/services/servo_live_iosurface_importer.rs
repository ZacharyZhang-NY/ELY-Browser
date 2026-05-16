#![cfg(target_os = "macos")]

use std::{
    io,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use core_video::pixel_buffer::CVPixelBuffer;

use crate::services::{
    iosurface_mach::IOSurfaceMachReceiver, iosurface_metal::import_pixel_buffer_from_mach_port,
};

use super::wire::LiveSurfaceHandle;

const RECEIVE_TIMEOUT: Duration = Duration::from_secs(1);

pub(super) struct IOSurfaceImportWorker {
    request_tx: Option<mpsc::Sender<LiveSurfaceHandle>>,
    result_rx: mpsc::Receiver<IOSurfaceImportResult>,
    thread: Option<JoinHandle<()>>,
}

impl IOSurfaceImportWorker {
    pub(super) fn new(receiver: IOSurfaceMachReceiver) -> Result<Self, io::Error> {
        let (request_tx, request_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("ely-iosurface-import".to_string())
            .spawn(move || run_import_worker(receiver, request_rx, result_tx))?;

        Ok(Self { request_tx: Some(request_tx), result_rx, thread: Some(thread) })
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
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixel_buffer: CVPixelBuffer,
}

// SAFETY: CVPixelBuffer is a CoreFoundation object with atomic
// retain/release semantics. This wrapper crosses from the importer
// thread to the live worker thread; GPUI presentation already receives
// the same handle through ServoLiveFrame's Send contract.
#[expect(unsafe_code)]
unsafe impl Send for ImportedIOSurface {}

pub(super) struct IOSurfaceImportFailure {
    pub(super) surface_id: u64,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) mach_port_name: u32,
    pub(super) message: String,
}

impl IOSurfaceImportFailure {
    fn worker_stopped(handle: LiveSurfaceHandle) -> Self {
        Self {
            surface_id: handle.surface_id,
            width: handle.width,
            height: handle.height,
            mach_port_name: handle.mach_port_name,
            message: "IOSurface import worker stopped".to_string(),
        }
    }
}

fn run_import_worker(
    mut receiver: IOSurfaceMachReceiver,
    request_rx: mpsc::Receiver<LiveSurfaceHandle>,
    result_tx: mpsc::Sender<IOSurfaceImportResult>,
) {
    while let Ok(handle) = request_rx.recv() {
        let result = import_surface_handle(&mut receiver, handle);
        if result_tx.send(result).is_err() {
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
                width: handle.width,
                height: handle.height,
                mach_port_name: handle.mach_port_name,
                message: error.to_string(),
            });
        }
    };

    match import_pixel_buffer_from_mach_port(mach_port_name) {
        Ok(pixel_buffer) => IOSurfaceImportResult::Imported(ImportedIOSurface {
            surface_id: handle.surface_id,
            width: handle.width,
            height: handle.height,
            pixel_buffer,
        }),
        Err(error) => IOSurfaceImportResult::Failed(IOSurfaceImportFailure {
            surface_id: handle.surface_id,
            width: handle.width,
            height: handle.height,
            mach_port_name,
            message: error.to_string(),
        }),
    }
}
