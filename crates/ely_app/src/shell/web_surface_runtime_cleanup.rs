use crate::services::servo_profile_data::TransientProfileDataDir;

use super::LiveRuntimeWorker;

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
