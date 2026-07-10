use ely_sync_client::{SyncClientError, SyncOwnerStore};

#[test]
fn browser_data_root_keeps_one_sync_owner() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = SyncOwnerStore::new(directory.path());

    assert!(matches!(store.verify("user-01"), Err(SyncClientError::SyncOwnerUnclaimed)));
    store.claim("user-01")?;
    store.claim("user-01")?;
    SyncOwnerStore::new(directory.path()).verify("user-01")?;

    assert!(matches!(store.verify("user-02"), Err(SyncClientError::SyncOwnerMismatch)));
    Ok(())
}

#[test]
fn malformed_sync_owner_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    write_owner(directory.path(), "invalid owner id!")?;
    let store = SyncOwnerStore::new(directory.path());

    assert!(matches!(store.verify("user-01"), Err(SyncClientError::SyncOwnerStorage(_))));
    Ok(())
}

#[test]
fn concurrent_claims_publish_exactly_one_owner() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = std::sync::Arc::new(directory.path().to_path_buf());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads = ["user-A", "user-B"].map(|user_id| {
        let root = root.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            SyncOwnerStore::new(&root).claim(user_id).is_ok()
        })
    });
    let outcomes = threads
        .into_iter()
        .map(|thread| thread.join().map_err(|_| "owner claim thread panicked"))
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(outcomes.iter().filter(|outcome| **outcome).count(), 1);
    let owner = std::fs::read_to_string(directory.path().join(".sync-owner-user-id"))?;
    assert!(matches!(owner.as_str(), "user-A\n" | "user-B\n"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn linked_owner_records_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir()?;
    let external = directory.path().join("external-owner");
    write_private_file(&external, "user-01\n")?;
    symlink(&external, directory.path().join(".sync-owner-user-id"))?;

    assert!(matches!(
        SyncOwnerStore::new(directory.path()).verify("user-01"),
        Err(SyncClientError::SyncOwnerStorage(_))
    ));
    Ok(())
}

fn write_owner(root: &std::path::Path, value: &str) -> Result<(), std::io::Error> {
    write_private_file(&root.join(".sync-owner-user-id"), value)
}

fn write_private_file(path: &std::path::Path, value: &str) -> Result<(), std::io::Error> {
    std::fs::write(path, value)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
