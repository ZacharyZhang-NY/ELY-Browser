use std::{ffi::CString, mem, time::Duration};

use mach2::{
    bootstrap::{bootstrap_look_up, bootstrap_port},
    kern_return::KERN_SUCCESS,
    mach_port::mach_port_deallocate,
    message::{
        MACH_MSG_SUCCESS, MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MOVE_SEND, MACH_MSGH_BITS,
        MACH_MSGH_BITS_COMPLEX, MACH_SEND_MSG, MACH_SEND_TIMEOUT, mach_msg, mach_msg_body_t,
        mach_msg_header_t, mach_msg_port_descriptor_t,
    },
    port::{MACH_PORT_NULL, mach_port_t},
    traps::mach_task_self,
};
use thiserror::Error;

use super::live_protocol::{LiveOutcome, LiveSidecarError};

const IOSURFACE_PORT_MESSAGE_ID: i32 = 0x454c_5901;
const SEND_TIMEOUT: Duration = Duration::from_secs(1);

pub(super) struct IOSurfaceMachSender {
    send_port: mach_port_t,
}

#[derive(Debug, Error)]
pub(super) enum IOSurfaceMachError {
    #[error("Mach service name contains an interior nul byte")]
    InvalidServiceName,

    #[error("bootstrap_look_up returned {code}")]
    LookupService { code: i32 },

    #[error("mach_msg send returned {code}")]
    Send { code: i32 },
}

impl IOSurfaceMachSender {
    pub(super) fn connect(service_name: &str) -> Result<Self, IOSurfaceMachError> {
        let service_name =
            CString::new(service_name).map_err(|_| IOSurfaceMachError::InvalidServiceName)?;
        let mut send_port = MACH_PORT_NULL;
        #[expect(unsafe_code)]
        let result =
            unsafe { bootstrap_look_up(bootstrap_port, service_name.as_ptr(), &mut send_port) };
        if result != KERN_SUCCESS {
            return Err(IOSurfaceMachError::LookupService { code: result });
        }
        Ok(Self { send_port })
    }

    pub(super) fn send_surface_port(
        &mut self,
        surface_id: u64,
        mach_port: mach_port_t,
    ) -> Result<(), IOSurfaceMachError> {
        let mut message = IOSurfacePortMessage {
            header: mach_msg_header_t {
                msgh_bits: MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0) | MACH_MSGH_BITS_COMPLEX,
                msgh_size: mem::size_of::<IOSurfacePortMessage>() as u32,
                msgh_remote_port: self.send_port,
                msgh_local_port: MACH_PORT_NULL,
                msgh_voucher_port: MACH_PORT_NULL,
                msgh_id: IOSURFACE_PORT_MESSAGE_ID,
            },
            body: mach_msg_body_t { msgh_descriptor_count: 1 },
            surface_port: mach_msg_port_descriptor_t::new(mach_port, MACH_MSG_TYPE_MOVE_SEND),
            surface_id,
        };
        #[expect(unsafe_code)]
        let result = unsafe {
            mach_msg(
                &mut message.header,
                MACH_SEND_MSG | MACH_SEND_TIMEOUT,
                message.header.msgh_size,
                0,
                MACH_PORT_NULL,
                timeout_millis(SEND_TIMEOUT),
                MACH_PORT_NULL,
            )
        };
        if result != MACH_MSG_SUCCESS {
            destroy_message(&mut message);
            return Err(IOSurfaceMachError::Send { code: result });
        }
        Ok(())
    }
}

impl Drop for IOSurfaceMachSender {
    fn drop(&mut self) {
        #[expect(unsafe_code)]
        let task = unsafe { mach_task_self() };
        #[expect(unsafe_code)]
        unsafe {
            let _ = mach_port_deallocate(task, self.send_port);
        }
    }
}

pub(super) fn send_surface_port_if_needed(
    sender: Option<&mut IOSurfaceMachSender>,
    outcome: &mut Result<LiveOutcome, LiveSidecarError>,
) {
    let (Some(sender), Ok(live_outcome)) = (sender, outcome.as_ref()) else {
        return;
    };
    let Some(handle) = live_outcome.response.surface_handle else {
        return;
    };
    if let Err(error) = sender.send_surface_port(handle.surface_id, handle.mach_port_name) {
        *outcome = Err(LiveSidecarError::IOSurfaceMach(error));
    }
}

#[repr(C)]
struct IOSurfacePortMessage {
    header: mach_msg_header_t,
    body: mach_msg_body_t,
    surface_port: mach_msg_port_descriptor_t,
    surface_id: u64,
}

fn timeout_millis(timeout: Duration) -> u32 {
    u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX).max(1)
}

fn destroy_message(message: &mut IOSurfacePortMessage) {
    #[expect(unsafe_code)]
    unsafe {
        mach2::message::mach_msg_destroy(&mut message.header);
    }
}
