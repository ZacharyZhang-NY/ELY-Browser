use ely_domain::ProfileId;

use super::SyncStateUpdate;

pub(in crate::shell) fn sync_failure_update(
    profile_id: ProfileId,
    error: ely_sync_client::SyncClientError,
) -> SyncStateUpdate {
    let message = error.to_string();
    match error {
        ely_sync_client::SyncClientError::SessionEnded
        | ely_sync_client::SyncClientError::SessionChanged => {
            SyncStateUpdate::AuthenticationExpired { profile_id }
        }
        ely_sync_client::SyncClientError::BearerCredentialStorage(_)
        | ely_sync_client::SyncClientError::AccountKeyStorage(_)
        | ely_sync_client::SyncClientError::DeviceKeyStorage(_) => {
            SyncStateUpdate::CredentialUnavailable { profile_id, message }
        }
        ely_sync_client::SyncClientError::DeviceApprovalStatus { .. } => {
            SyncStateUpdate::AwaitingDeviceApproval { profile_id, finishes_upload: true }
        }
        _ if message.contains("device_not_approved") => {
            SyncStateUpdate::AwaitingDeviceApproval { profile_id, finishes_upload: true }
        }
        _ => SyncStateUpdate::SyncError { profile_id, message },
    }
}

pub(in crate::shell) fn device_failure_update(
    profile_id: ProfileId,
    error: ely_sync_client::SyncClientError,
) -> SyncStateUpdate {
    match sync_failure_update(profile_id, error) {
        SyncStateUpdate::AwaitingDeviceApproval { profile_id, .. } => {
            SyncStateUpdate::AwaitingDeviceApproval { profile_id, finishes_upload: false }
        }
        SyncStateUpdate::SyncError { profile_id, message } => {
            SyncStateUpdate::DevicesError { profile_id, message }
        }
        update => update,
    }
}
