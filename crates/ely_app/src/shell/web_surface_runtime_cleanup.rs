use std::path::PathBuf;

use crate::services::{servo_live::ServoLiveClient, servo_profile_data::TransientProfileDataDir};

use super::{LiveRuntimeClient, LiveRuntimeWorker};

pub(super) type LiveRuntimeClientFactory =
    fn(PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String>;

pub(super) fn new_servo_live_client(
    config_dir: PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    ServoLiveClient::new(config_dir)
        .map(|client| Box::new(client) as Box<dyn LiveRuntimeClient>)
        .map_err(|error| error.to_string())
}

pub(super) struct ScopedWorker {
    pub(super) worker: LiveRuntimeWorker,
    pub(super) transient_profile_data_dir: Option<TransientProfileDataDir>,
}

pub(super) fn shutdown_scoped_worker(scoped: ScopedWorker) -> Result<(), String> {
    let ScopedWorker { worker, transient_profile_data_dir } = scoped;
    drop(worker);
    let Some(directory) = transient_profile_data_dir else {
        return Ok(());
    };
    directory
        .close()
        .map_err(|error| format!("failed to remove transient Servo profile data: {error}"))
}
