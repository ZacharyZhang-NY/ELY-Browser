use std::{error::Error, ffi::CString, io, mem, time::Duration};

use mach2::{
    bootstrap::{bootstrap_port, bootstrap_register},
    kern_return::KERN_SUCCESS,
    mach_port::{
        mach_port_allocate, mach_port_deallocate, mach_port_destroy, mach_port_insert_right,
    },
    message::{
        MACH_MSG_PORT_DESCRIPTOR, MACH_MSG_SUCCESS, MACH_MSG_TYPE_MAKE_SEND, MACH_RCV_MSG,
        MACH_RCV_TIMED_OUT, MACH_RCV_TIMEOUT, mach_msg, mach_msg_body_t, mach_msg_header_t,
        mach_msg_port_descriptor_t, mach_msg_trailer_t,
    },
    port::{MACH_PORT_NULL, MACH_PORT_RIGHT_RECEIVE, mach_port_t},
    traps::mach_task_self,
};

const IOSURFACE_PORT_MESSAGE_ID: i32 = 0x454c_5901;

pub(super) struct MachSurfaceReceiver {
    service_name: String,
    receive_port: mach_port_t,
}

impl MachSurfaceReceiver {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let service_name =
            format!("com.ely.browser.iosurface.test.{}", ely_domain::ProfileId::new().as_str());
        let service_name_c = CString::new(service_name.as_str())?;
        let mut receive_port = MACH_PORT_NULL;
        #[expect(unsafe_code)]
        let task = unsafe { mach_task_self() };
        #[expect(unsafe_code)]
        let allocate =
            unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &mut receive_port) };
        if allocate != KERN_SUCCESS {
            return Err(io::Error::other(format!("mach_port_allocate returned {allocate}")).into());
        }
        #[expect(unsafe_code)]
        let insert = unsafe {
            mach_port_insert_right(task, receive_port, receive_port, MACH_MSG_TYPE_MAKE_SEND)
        };
        if insert != KERN_SUCCESS {
            destroy_port(receive_port);
            return Err(
                io::Error::other(format!("mach_port_insert_right returned {insert}")).into()
            );
        }
        #[expect(unsafe_code)]
        #[allow(deprecated)]
        let register = unsafe {
            bootstrap_register(bootstrap_port, service_name_c.as_ptr() as *mut _, receive_port)
        };
        if register != KERN_SUCCESS {
            destroy_port(receive_port);
            return Err(io::Error::other(format!("bootstrap_register returned {register}")).into());
        }
        Ok(Self { service_name, receive_port })
    }

    pub(super) fn service_name(&self) -> &str {
        self.service_name.as_str()
    }

    pub(super) fn receive(
        &self,
        expected_surface_id: u64,
        timeout: Duration,
    ) -> Result<mach_port_t, Box<dyn Error>> {
        #[expect(unsafe_code)]
        let mut received: ReceivedIOSurfacePortMessage = unsafe { mem::zeroed() };
        #[expect(unsafe_code)]
        let result = unsafe {
            mach_msg(
                &mut received.message.header,
                MACH_RCV_MSG | MACH_RCV_TIMEOUT,
                0,
                mem::size_of::<ReceivedIOSurfacePortMessage>() as u32,
                self.receive_port,
                timeout_millis(timeout),
                MACH_PORT_NULL,
            )
        };
        if result == MACH_RCV_TIMED_OUT {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Mach receive timed out for surface {expected_surface_id}"),
            )
            .into());
        }
        if result != MACH_MSG_SUCCESS {
            return Err(io::Error::other(format!("mach_msg receive returned {result}")).into());
        }
        let message = &mut received.message;
        if message.header.msgh_id != IOSURFACE_PORT_MESSAGE_ID
            || message.body.msgh_descriptor_count != 1
            || message.surface_port.type_ != MACH_MSG_PORT_DESCRIPTOR as u8
            || message.surface_port.name == MACH_PORT_NULL
            || message.surface_id != expected_surface_id
        {
            destroy_message(message);
            return Err(io::Error::other("received invalid IOSurface Mach message").into());
        }
        Ok(message.surface_port.name)
    }
}

impl Drop for MachSurfaceReceiver {
    fn drop(&mut self) {
        destroy_port(self.receive_port);
    }
}

pub(super) fn verify_iosurface(
    mach_port: mach_port_t,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn Error>> {
    let surface = objc2_io_surface::IOSurfaceRef::lookup_from_mach_port(mach_port)
        .ok_or_else(|| io::Error::other("IOSurfaceLookupFromMachPort returned null"))?;
    let actual_width = u32::try_from(surface.width())?;
    let actual_height = u32::try_from(surface.height())?;
    deallocate_port(mach_port)?;
    if actual_width != width || actual_height != height {
        return Err(io::Error::other(format!(
            "imported IOSurface was {actual_width}x{actual_height}; expected {width}x{height}"
        ))
        .into());
    }
    Ok(())
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

fn timeout_millis(timeout: Duration) -> u32 {
    u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX).max(1)
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

fn deallocate_port(port: mach_port_t) -> Result<(), Box<dyn Error>> {
    #[expect(unsafe_code)]
    let task = unsafe { mach_task_self() };
    #[expect(unsafe_code)]
    let result = unsafe { mach_port_deallocate(task, port) };
    if result != KERN_SUCCESS {
        return Err(io::Error::other(format!("mach_port_deallocate returned {result}")).into());
    }
    Ok(())
}
