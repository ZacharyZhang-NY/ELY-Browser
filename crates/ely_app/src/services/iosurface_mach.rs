use std::{
    collections::BTreeMap,
    ffi::CString,
    mem,
    time::{Duration, Instant},
};

use mach2::{
    bootstrap::{bootstrap_port, bootstrap_register},
    kern_return::KERN_SUCCESS,
    mach_port::{mach_port_allocate, mach_port_destroy, mach_port_insert_right},
    message::{
        MACH_MSG_PORT_DESCRIPTOR, MACH_MSG_SUCCESS, MACH_MSG_TIMEOUT_NONE, MACH_MSG_TYPE_MAKE_SEND,
        MACH_RCV_MSG, MACH_RCV_TIMED_OUT, MACH_RCV_TIMEOUT, mach_msg, mach_msg_body_t,
        mach_msg_header_t, mach_msg_port_descriptor_t, mach_msg_trailer_t,
    },
    port::{MACH_PORT_NULL, MACH_PORT_RIGHT_RECEIVE, mach_port_t},
    traps::mach_task_self,
};
use thiserror::Error;
use uuid::Uuid;

const IOSURFACE_PORT_MESSAGE_ID: i32 = 0x454c_5901;
const SERVICE_PREFIX: &str = "com.ely.browser.iosurface";

pub(crate) struct IOSurfaceMachReceiver {
    service_name: String,
    receive_port: mach_port_t,
    pending_ports: BTreeMap<u64, mach_port_t>,
}

#[derive(Debug, Error)]
pub(crate) enum IOSurfaceMachError {
    #[error("Mach service name contains an interior nul byte")]
    InvalidServiceName,

    #[error("mach_port_allocate returned {code}")]
    AllocatePort { code: i32 },

    #[error("mach_port_insert_right returned {code}")]
    InsertSendRight { code: i32 },

    #[error("bootstrap_register returned {code}")]
    RegisterService { code: i32 },

    #[error("mach_msg receive timed out for IOSurface surface {surface_id:#x}")]
    ReceiveTimedOut { surface_id: u64 },

    #[error("mach_msg receive returned {code}")]
    Receive { code: i32 },

    #[error("received unexpected Mach message id {message_id}")]
    UnexpectedMessage { message_id: i32 },

    #[error("received invalid IOSurface Mach message")]
    InvalidMessage,
}

impl IOSurfaceMachReceiver {
    pub(crate) fn new() -> Result<Self, IOSurfaceMachError> {
        let service_name = unique_service_name();
        let service_name_c = CString::new(service_name.as_str())
            .map_err(|_| IOSurfaceMachError::InvalidServiceName)?;
        let mut receive_port = MACH_PORT_NULL;

        #[expect(unsafe_code)]
        let task = unsafe { mach_task_self() };
        #[expect(unsafe_code)]
        let allocate =
            unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &mut receive_port) };
        if allocate != KERN_SUCCESS {
            return Err(IOSurfaceMachError::AllocatePort { code: allocate });
        }

        #[expect(unsafe_code)]
        let insert = unsafe {
            mach_port_insert_right(task, receive_port, receive_port, MACH_MSG_TYPE_MAKE_SEND)
        };
        if insert != KERN_SUCCESS {
            destroy_port(receive_port);
            return Err(IOSurfaceMachError::InsertSendRight { code: insert });
        }

        #[expect(unsafe_code)]
        #[allow(deprecated)]
        let register = unsafe {
            bootstrap_register(bootstrap_port, service_name_c.as_ptr() as *mut _, receive_port)
        };
        if register != KERN_SUCCESS {
            destroy_port(receive_port);
            return Err(IOSurfaceMachError::RegisterService { code: register });
        }

        Ok(Self { service_name, receive_port, pending_ports: BTreeMap::new() })
    }

    pub(crate) fn service_name(&self) -> &str {
        self.service_name.as_str()
    }

    pub(crate) fn receive_port_for_surface(
        &mut self,
        surface_id: u64,
        timeout: Duration,
    ) -> Result<mach_port_t, IOSurfaceMachError> {
        if let Some(port) = self.pending_ports.remove(&surface_id) {
            return Ok(port);
        }

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(IOSurfaceMachError::ReceiveTimedOut { surface_id });
            }
            let Some(received) = self.receive_one(remaining)? else {
                return Err(IOSurfaceMachError::ReceiveTimedOut { surface_id });
            };
            if received.surface_id == surface_id {
                return Ok(received.mach_port);
            }
            self.pending_ports.insert(received.surface_id, received.mach_port);
        }
    }

    fn receive_one(
        &self,
        timeout: Duration,
    ) -> Result<Option<ReceivedSurfacePort>, IOSurfaceMachError> {
        #[expect(unsafe_code)]
        let mut received_message: ReceivedIOSurfacePortMessage = unsafe { mem::zeroed() };
        let timeout_ms = timeout_millis(timeout);
        #[expect(unsafe_code)]
        let result = unsafe {
            mach_msg(
                &mut received_message.message.header,
                MACH_RCV_MSG | MACH_RCV_TIMEOUT,
                0,
                mem::size_of::<ReceivedIOSurfacePortMessage>() as u32,
                self.receive_port,
                timeout_ms,
                MACH_PORT_NULL,
            )
        };

        if result == MACH_RCV_TIMED_OUT {
            return Ok(None);
        }
        if result != MACH_MSG_SUCCESS {
            return Err(IOSurfaceMachError::Receive { code: result });
        }
        let message = &mut received_message.message;
        if message.header.msgh_id != IOSURFACE_PORT_MESSAGE_ID {
            destroy_message(message);
            return Err(IOSurfaceMachError::UnexpectedMessage {
                message_id: message.header.msgh_id,
            });
        }
        if message.body.msgh_descriptor_count != 1
            || message.surface_port.type_ != MACH_MSG_PORT_DESCRIPTOR as u8
            || message.surface_port.name == MACH_PORT_NULL
        {
            destroy_message(message);
            return Err(IOSurfaceMachError::InvalidMessage);
        }

        Ok(Some(ReceivedSurfacePort {
            surface_id: message.surface_id,
            mach_port: message.surface_port.name,
        }))
    }
}

impl Drop for IOSurfaceMachReceiver {
    fn drop(&mut self) {
        for port in std::mem::take(&mut self.pending_ports).into_values() {
            deallocate_port(port);
        }
        destroy_port(self.receive_port);
    }
}

struct ReceivedSurfacePort {
    surface_id: u64,
    mach_port: mach_port_t,
}

#[repr(C)]
struct IOSurfacePortMessage {
    header: mach_msg_header_t,
    body: mach_msg_body_t,
    surface_port: mach_msg_port_descriptor_t,
    surface_id: u64,
}

#[repr(C)]
struct ReceivedIOSurfacePortMessage {
    message: IOSurfacePortMessage,
    _trailer: mach_msg_trailer_t,
}

fn unique_service_name() -> String {
    format!("{SERVICE_PREFIX}.{}", Uuid::now_v7().as_simple())
}

fn timeout_millis(timeout: Duration) -> u32 {
    u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX).max(MACH_MSG_TIMEOUT_NONE + 1)
}

fn destroy_message(message: &mut IOSurfacePortMessage) {
    #[expect(unsafe_code)]
    unsafe {
        mach2::message::mach_msg_destroy(&mut message.header);
    }
}

fn destroy_port(port: mach_port_t) {
    #[expect(unsafe_code)]
    let task = unsafe { mach_task_self() };
    #[expect(unsafe_code)]
    unsafe {
        let _ = mach_port_destroy(task, port);
    }
}

fn deallocate_port(port: mach_port_t) {
    #[expect(unsafe_code)]
    let task = unsafe { mach_task_self() };
    #[expect(unsafe_code)]
    unsafe {
        let _ = mach2::mach_port::mach_port_deallocate(task, port);
    }
}
