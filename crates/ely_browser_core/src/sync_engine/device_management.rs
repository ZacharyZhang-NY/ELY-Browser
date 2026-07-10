use ely_sync_client::{
    AccountKey, DeviceApprovalDocument, DeviceApprovalRequest, DeviceListResponse, DeviceRecord,
    DeviceRevocationDocument, DeviceRevocationRequest, SyncApiClient, SyncClientError,
    VaultContext, WrappedAccountKey,
};
use uuid::Uuid;

use super::SyncEngine;

impl SyncEngine {
    pub fn cloud_devices(&self) -> Result<DeviceListResponse, SyncClientError> {
        self.with_required_authenticated_client(|client| self.cloud_devices_with_client(client))
    }

    fn cloud_devices_with_client(
        &self,
        client: SyncApiClient,
    ) -> Result<DeviceListResponse, SyncClientError> {
        let (client, registration) = self.registered_client(client)?;
        let devices = client.list_devices()?;
        validate_device_list(
            &devices,
            &registration.user_id,
            &self.identity.device_id,
            registration.device.is_approved(),
        )?;
        Ok(devices)
    }

    pub fn approve_cloud_device(
        &self,
        target_device_id: &str,
        verification_code: &str,
    ) -> Result<DeviceApprovalDocument, SyncClientError> {
        self.with_required_authenticated_client(|client| {
            self.approve_cloud_device_with_client(client, target_device_id, verification_code)
        })
    }

    fn approve_cloud_device_with_client(
        &self,
        client: SyncApiClient,
        target_device_id: &str,
        verification_code: &str,
    ) -> Result<DeviceApprovalDocument, SyncClientError> {
        let (client, user_id) = self.approved_device_client(client)?;
        let devices = client.list_devices()?;
        validate_device_list(&devices, &user_id, &self.identity.device_id, true)?;
        let target = pending_device(&devices.devices, target_device_id)?;
        if !target.verification_code()?.eq_ignore_ascii_case(verification_code.trim()) {
            return Err(SyncClientError::DeviceTrust {
                reason: "device verification code does not match",
            });
        }
        let wrapping_public_key =
            target.wrapping_public_key.as_deref().ok_or(SyncClientError::DeviceTrust {
                reason: "pending device has no wrapping public key",
            })?;

        let vault = self.resolve_vault(&client, &user_id)?;
        let key_id = vault.account_key.key_id();
        let envelope = WrappedAccountKey::wrap(
            &vault.account_key,
            &VaultContext {
                user_id: &user_id,
                recipient_device_id: &target.device_id,
                recipient_wrapping_public_key: wrapping_public_key,
                approver_device_id: &self.identity.device_id,
                generation: vault.generation,
                key_id: &key_id,
            },
        )?;
        let idempotency_key = format!("device-approval:{}", Uuid::now_v7().simple());
        let request = DeviceApprovalRequest::new(
            &user_id,
            &self.identity,
            &target.device_id,
            &key_id,
            vault.generation,
            &envelope,
            &idempotency_key,
        )?;
        let document = client.approve_device(&request)?;
        validate_approval(&document, &user_id, &self.identity.device_id, &target.device_id)?;
        Ok(document)
    }

    pub fn revoke_cloud_device(
        &self,
        target_device_id: &str,
    ) -> Result<DeviceRevocationDocument, SyncClientError> {
        self.with_required_authenticated_client(|client| {
            self.revoke_cloud_device_with_client(client, target_device_id)
        })
    }

    fn revoke_cloud_device_with_client(
        &self,
        client: SyncApiClient,
        target_device_id: &str,
    ) -> Result<DeviceRevocationDocument, SyncClientError> {
        let (client, user_id) = self.approved_device_client(client)?;
        let devices = client.list_devices()?;
        validate_device_list(&devices, &user_id, &self.identity.device_id, true)?;
        let target = revocable_device(&devices.devices, target_device_id)?;
        let idempotency_key = format!("device-revocation:{}", Uuid::now_v7().simple());
        if target.approval_status == "pending" {
            let request = DeviceRevocationRequest::pending(
                &user_id,
                &self.identity,
                &target.device_id,
                &idempotency_key,
            )?;
            let document = client.revoke_device(&request)?;
            validate_pending_revocation(
                &document,
                &user_id,
                &self.identity.device_id,
                &target.device_id,
            )?;
            return Ok(document);
        }
        let vault = self.resolve_vault(&client, &user_id)?;
        let previous_key_id = vault.account_key.key_id();
        let new_generation =
            vault.generation.checked_add(1).ok_or(SyncClientError::DeviceTrust {
                reason: "device revocation generation overflowed",
            })?;
        let new_key = AccountKey::generate()?;
        let new_key_id = new_key.key_id();
        let envelopes = rotation_envelopes(
            &devices.devices,
            &target.device_id,
            &user_id,
            &self.identity.device_id,
            &new_key,
            new_generation,
            &new_key_id,
        )?;
        let request = DeviceRevocationRequest::approved_rotation(
            &user_id,
            &self.identity,
            &target.device_id,
            &previous_key_id,
            vault.generation,
            &new_key_id,
            new_generation,
            envelopes,
            &idempotency_key,
        )?;
        let document = client.revoke_device(&request)?;
        validate_approved_revocation(
            &document,
            &user_id,
            &self.identity.device_id,
            &target.device_id,
            &new_key_id,
            new_generation,
        )?;
        self.account_key_store(&user_id)?.save_current(&new_key, new_generation)?;
        Ok(document)
    }

    fn approved_device_client(
        &self,
        client: SyncApiClient,
    ) -> Result<(SyncApiClient, String), SyncClientError> {
        self.approved_client(client)?.ok_or_else(|| SyncClientError::DeviceApprovalStatus {
            device_id: self.identity.device_id.clone(),
            status: "pending".to_string(),
        })
    }
}

fn validate_device_list(
    document: &DeviceListResponse,
    user_id: &str,
    current_device_id: &str,
    require_approved: bool,
) -> Result<(), SyncClientError> {
    let current = document.devices.iter().filter(|device| device.current).collect::<Vec<_>>();
    if document.version != 1
        || document.user_id != user_id
        || current.len() != 1
        || current[0].device_id != current_device_id
        || (require_approved && !current[0].is_approved())
    {
        return Err(SyncClientError::DeviceTrust {
            reason: "device list does not match the authenticated device",
        });
    }
    Ok(())
}

fn pending_device<'a>(
    devices: &'a [DeviceRecord],
    target_device_id: &str,
) -> Result<&'a DeviceRecord, SyncClientError> {
    let target = devices
        .iter()
        .find(|device| device.device_id == target_device_id)
        .ok_or(SyncClientError::DeviceTrust { reason: "pending device was not found" })?;
    if target.current || target.approval_status != "pending" || target.revoked_at.is_some() {
        return Err(SyncClientError::DeviceTrust { reason: "device is not pending approval" });
    }
    Ok(target)
}

fn revocable_device<'a>(
    devices: &'a [DeviceRecord],
    target_device_id: &str,
) -> Result<&'a DeviceRecord, SyncClientError> {
    let target = devices
        .iter()
        .find(|device| device.device_id == target_device_id)
        .ok_or(SyncClientError::DeviceTrust { reason: "device was not found" })?;
    if target.current
        || target.revoked_at.is_some()
        || !matches!(target.approval_status.as_str(), "pending" | "approved")
    {
        return Err(SyncClientError::DeviceTrust { reason: "device cannot be revoked" });
    }
    Ok(target)
}

#[allow(clippy::too_many_arguments)]
fn rotation_envelopes(
    devices: &[DeviceRecord],
    target_device_id: &str,
    user_id: &str,
    approver_device_id: &str,
    new_key: &AccountKey,
    new_generation: u64,
    new_key_id: &str,
) -> Result<Vec<(String, WrappedAccountKey)>, SyncClientError> {
    devices
        .iter()
        .filter(|device| device.device_id != target_device_id && device.is_approved())
        .filter_map(|device| {
            device
                .wrapping_public_key
                .as_deref()
                .map(|wrapping_public_key| (device, wrapping_public_key))
        })
        .map(|(device, wrapping_public_key)| {
            let envelope = WrappedAccountKey::wrap(
                new_key,
                &VaultContext {
                    user_id,
                    recipient_device_id: &device.device_id,
                    recipient_wrapping_public_key: wrapping_public_key,
                    approver_device_id,
                    generation: new_generation,
                    key_id: new_key_id,
                },
            )?;
            Ok((device.device_id.clone(), envelope))
        })
        .collect()
}

fn validate_approval(
    document: &DeviceApprovalDocument,
    user_id: &str,
    approver_device_id: &str,
    target_device_id: &str,
) -> Result<(), SyncClientError> {
    if document.version != 1
        || document.user_id != user_id
        || document.approved_by_device_id != approver_device_id
        || document.device.device_id != target_device_id
        || !document.device.is_approved()
        || document.device.approved_at != Some(document.approved_at)
    {
        return Err(SyncClientError::DeviceTrust {
            reason: "device approval response does not match the request",
        });
    }
    Ok(())
}

fn validate_approved_revocation(
    document: &DeviceRevocationDocument,
    user_id: &str,
    approver_device_id: &str,
    target_device_id: &str,
    key_id: &str,
    generation: u64,
) -> Result<(), SyncClientError> {
    let DeviceRevocationDocument::ApprovedRotate {
        version,
        user_id: response_user_id,
        revoked_by_device_id,
        revoked_at,
        key_id: response_key_id,
        generation: response_generation,
        device,
    } = document
    else {
        return Err(revocation_response_error());
    };
    if response_key_id != key_id || *response_generation != generation {
        return Err(revocation_response_error());
    }
    validate_revocation_common(
        *version,
        response_user_id,
        revoked_by_device_id,
        *revoked_at,
        device,
        user_id,
        approver_device_id,
        target_device_id,
    )
}

fn validate_pending_revocation(
    document: &DeviceRevocationDocument,
    user_id: &str,
    approver_device_id: &str,
    target_device_id: &str,
) -> Result<(), SyncClientError> {
    let DeviceRevocationDocument::PendingRevoke {
        version,
        user_id: response_user_id,
        revoked_by_device_id,
        revoked_at,
        device,
    } = document
    else {
        return Err(revocation_response_error());
    };
    validate_revocation_common(
        *version,
        response_user_id,
        revoked_by_device_id,
        *revoked_at,
        device,
        user_id,
        approver_device_id,
        target_device_id,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_revocation_common(
    version: u32,
    response_user_id: &str,
    revoked_by_device_id: &str,
    revoked_at: u64,
    device: &DeviceRecord,
    user_id: &str,
    approver_device_id: &str,
    target_device_id: &str,
) -> Result<(), SyncClientError> {
    if version == 2
        && response_user_id == user_id
        && revoked_by_device_id == approver_device_id
        && device.device_id == target_device_id
        && device.approval_status == "revoked"
        && device.revoked_at == Some(revoked_at)
        && !device.current
    {
        return Ok(());
    }
    Err(revocation_response_error())
}

fn revocation_response_error() -> SyncClientError {
    SyncClientError::DeviceTrust { reason: "device revocation response does not match the request" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(device_id: &str, status: &str, current: bool) -> DeviceRecord {
        DeviceRecord {
            device_id: device_id.to_string(),
            public_key: "01".repeat(32),
            wrapping_public_key: Some("02".repeat(32)),
            device_name: "Test".to_string(),
            platform: "macos".to_string(),
            approval_status: status.to_string(),
            current,
            created_at: 1,
            approved_at: (status == "approved").then_some(2),
            last_active_at: None,
            revoked_at: None,
        }
    }

    #[test]
    fn device_list_requires_one_approved_current_device() {
        let document = DeviceListResponse {
            version: 1,
            user_id: "user-01".to_string(),
            devices: vec![device("device-01", "approved", true)],
        };
        assert!(validate_device_list(&document, "user-01", "device-01", true).is_ok());
        assert!(validate_device_list(&document, "user-02", "device-01", true).is_err());
    }

    #[test]
    fn approval_target_must_be_pending() {
        let devices =
            [device("device-01", "approved", true), device("device-02", "pending", false)];
        assert!(matches!(
            pending_device(&devices, "device-02"),
            Ok(device) if device.device_id == "device-02"
        ));
        assert!(pending_device(&devices, "device-01").is_err());
    }

    #[test]
    fn approval_response_requires_matching_device() {
        let device = device("device-02", "approved", false);
        let document = DeviceApprovalDocument {
            version: 1,
            user_id: "user-01".to_string(),
            approved_by_device_id: "device-01".to_string(),
            approved_at: 2,
            device,
        };
        assert!(validate_approval(&document, "user-01", "device-01", "device-02").is_ok());
        assert!(validate_approval(&document, "user-01", "device-01", "device-03").is_err());
    }

    #[test]
    fn revocation_target_must_be_another_active_device() {
        let devices = [
            device("device-01", "approved", true),
            device("device-02", "approved", false),
            device("device-03", "revoked", false),
        ];
        assert!(matches!(
            revocable_device(&devices, "device-02"),
            Ok(device) if device.device_id == "device-02"
        ));
        assert!(revocable_device(&devices, "device-01").is_err());
        assert!(revocable_device(&devices, "device-03").is_err());
    }

    #[test]
    fn revocation_response_binds_rotated_key_and_target() {
        let mut revoked = device("device-02", "revoked", false);
        revoked.approved_at = Some(2);
        revoked.revoked_at = Some(3);
        let document = DeviceRevocationDocument::ApprovedRotate {
            version: 2,
            user_id: "user-01".to_string(),
            revoked_by_device_id: "device-01".to_string(),
            revoked_at: 3,
            key_id: "03".repeat(32),
            generation: 2,
            device: revoked,
        };
        assert!(
            validate_approved_revocation(
                &document,
                "user-01",
                "device-01",
                "device-02",
                &"03".repeat(32),
                2,
            )
            .is_ok()
        );
        assert!(
            validate_approved_revocation(
                &document,
                "user-01",
                "device-01",
                "device-03",
                &"03".repeat(32),
                2,
            )
            .is_err()
        );
    }
}
