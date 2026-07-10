use thiserror::Error;

#[path = "ely_servo_sidecar/args.rs"]
mod args;
#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[path = "ely_servo_sidecar/iosurface_mach.rs"]
mod iosurface_mach;
#[path = "ely_servo_sidecar/live.rs"]
mod live;
#[path = "ely_servo_sidecar/live_output.rs"]
mod live_output;
#[path = "ely_servo_sidecar/live_protocol.rs"]
mod live_protocol;
#[path = "ely_servo_sidecar/live_request.rs"]
mod live_request;
#[path = "ely_servo_sidecar/live_session.rs"]
mod live_session;
#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[path = "ely_servo_sidecar/live_surface.rs"]
mod live_surface;

use args::SidecarCommand;

fn main() -> Result<(), SidecarError> {
    match args::parse_env_command()? {
        SidecarCommand::Live(args) => live::run(args)?,
    }
    Ok(())
}

#[derive(Debug, Error)]
enum SidecarError {
    #[error(transparent)]
    Args(#[from] args::SidecarArgsError),

    #[error(transparent)]
    Live(#[from] live_protocol::LiveSidecarError),
}
