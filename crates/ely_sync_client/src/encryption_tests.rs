use super::*;

const CONTEXT: SnapshotCryptoContext<'static> = SnapshotCryptoContext {
    user_id: "user-01",
    vault_generation: 1,
    snapshot_id: "device-01",
    schema_rev: 1,
    logical_clock: 42,
    device_id: "device-01",
    head_revision: 1,
    base_head: None,
};

#[test]
fn snapshot_encryption_round_trips_and_hides_plaintext() -> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([7; 32]);
    let plaintext = br#"{"tabs":[{"url":"https://private.example"}]}"#;
    let encrypted = key.encrypt(&CONTEXT, plaintext)?;

    assert!(!encrypted.bytes().windows(plaintext.len()).any(|window| window == plaintext));
    assert_eq!(
        key.decrypt(
            &CONTEXT,
            SNAPSHOT_ENCRYPTION_VERSION,
            encrypted.key_id(),
            encrypted.content_hash(),
            encrypted.bytes(),
        )?,
        plaintext
    );
    Ok(())
}

#[test]
fn snapshot_encryption_uses_fresh_nonces_and_stable_keyed_content_hashes()
-> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([9; 32]);
    let first = key.encrypt(&CONTEXT, b"same payload")?;
    let second = key.encrypt(&CONTEXT, b"same payload")?;

    assert_ne!(first.bytes(), second.bytes());
    assert_eq!(first.content_hash(), second.content_hash());
    assert_ne!(
        first.content_hash(),
        AccountKey::from_bytes([10; 32]).content_hash(b"same payload")?
    );
    Ok(())
}

#[test]
fn snapshot_authentication_rejects_metadata_and_ciphertext_tampering() -> Result<(), SyncClientError>
{
    let key = AccountKey::from_bytes([11; 32]);
    let encrypted = key.encrypt(&CONTEXT, b"authenticated payload")?;
    let changed_context = SnapshotCryptoContext { logical_clock: 43, ..CONTEXT };

    assert!(
        key.decrypt(
            &changed_context,
            SNAPSHOT_ENCRYPTION_VERSION,
            encrypted.key_id(),
            encrypted.content_hash(),
            encrypted.bytes(),
        )
        .is_err()
    );

    let mut tampered = encrypted.bytes().to_vec();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(
        key.decrypt(
            &CONTEXT,
            SNAPSHOT_ENCRYPTION_VERSION,
            encrypted.key_id(),
            encrypted.content_hash(),
            &tampered,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn snapshot_authentication_binds_every_routing_field() -> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([13; 32]);
    let encrypted = key.encrypt(&CONTEXT, b"routing metadata")?;
    let changed = [
        SnapshotCryptoContext { user_id: "user-02", ..CONTEXT },
        SnapshotCryptoContext { vault_generation: 2, ..CONTEXT },
        SnapshotCryptoContext { snapshot_id: "device-02", ..CONTEXT },
        SnapshotCryptoContext { schema_rev: 2, ..CONTEXT },
        SnapshotCryptoContext { logical_clock: 41, ..CONTEXT },
        SnapshotCryptoContext { device_id: "device-02", ..CONTEXT },
    ];

    for context in changed {
        assert!(
            key.decrypt(
                &context,
                SNAPSHOT_ENCRYPTION_VERSION,
                encrypted.key_id(),
                encrypted.content_hash(),
                encrypted.bytes(),
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn snapshot_authentication_binds_head_lineage() -> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([14; 32]);
    let base = SnapshotHeadRef::new(7, "device-02", "31".repeat(32))?;
    let context = SnapshotCryptoContext { head_revision: 8, base_head: Some(&base), ..CONTEXT };
    let encrypted = key.encrypt(&context, b"head lineage")?;
    for changed_base in [
        SnapshotHeadRef::new(6, "device-02", "31".repeat(32))?,
        SnapshotHeadRef::new(7, "device-03", "31".repeat(32))?,
        SnapshotHeadRef::new(7, "device-02", "32".repeat(32))?,
    ] {
        let changed = SnapshotCryptoContext {
            head_revision: changed_base.revision + 1,
            base_head: Some(&changed_base),
            ..CONTEXT
        };
        assert!(
            key.decrypt(
                &changed,
                SNAPSHOT_ENCRYPTION_VERSION,
                encrypted.key_id(),
                encrypted.content_hash(),
                encrypted.bytes(),
            )
            .is_err()
        );
    }
    let changed_revision = SnapshotCryptoContext { head_revision: 9, ..context };
    assert!(
        key.decrypt(
            &changed_revision,
            SNAPSHOT_ENCRYPTION_VERSION,
            encrypted.key_id(),
            encrypted.content_hash(),
            encrypted.bytes(),
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn legacy_v1_aad_remains_decryptable() -> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([16; 32]);
    let encrypted = key.encrypt_with_version(&CONTEXT, b"legacy snapshot", 1)?;
    assert_eq!(
        key.decrypt(&CONTEXT, 1, encrypted.key_id(), encrypted.content_hash(), encrypted.bytes(),)?,
        b"legacy snapshot"
    );
    Ok(())
}

#[test]
fn snapshot_envelope_rejects_unknown_versions_and_wrong_keys() -> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([15; 32]);
    let encrypted = key.encrypt(&CONTEXT, b"versioned payload")?;

    assert!(
        key.decrypt(
            &CONTEXT,
            SNAPSHOT_ENCRYPTION_VERSION + 1,
            encrypted.key_id(),
            encrypted.content_hash(),
            encrypted.bytes(),
        )
        .is_err()
    );
    assert!(
        AccountKey::from_bytes([16; 32])
            .decrypt(
                &CONTEXT,
                SNAPSHOT_ENCRYPTION_VERSION,
                encrypted.key_id(),
                encrypted.content_hash(),
                encrypted.bytes(),
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn snapshot_envelope_honors_the_transport_size_limit() -> Result<(), SyncClientError> {
    let key = AccountKey::from_bytes([17; 32]);
    let maximum = vec![0_u8; MAX_PLAINTEXT_BYTES];
    let encrypted = key.encrypt(&CONTEXT, &maximum)?;

    assert_eq!(encrypted.bytes().len(), MAX_SNAPSHOT_BYTES);
    assert!(key.encrypt(&CONTEXT, &vec![0_u8; MAX_PLAINTEXT_BYTES + 1]).is_err());
    Ok(())
}
