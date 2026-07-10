use std::{fs, path::Path};

use super::*;

const PUBLIC_KEY: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

#[test]
fn copies_only_valid_device_once() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source = source_dir(directory.path());
    let stable = stable_dir(directory.path());
    fs::create_dir_all(source.join("nested"))?;
    fs::write(source.join(DEVICE_FILE), device_bytes("Legacy"))?;
    fs::write(source.join("bearer.token"), "secret")?;
    fs::write(source.join("other.json"), "other")?;
    migrate_default_sync_device(directory.path(), &stable)?;
    let destination = stable.join("sync");
    assert_eq!(fs::read(destination.join(DEVICE_FILE))?, device_bytes("Legacy"));
    assert!(!destination.join("bearer.token").exists());
    assert!(!destination.join("other.json").exists());
    assert!(!destination.join("nested").exists());
    fs::remove_file(destination.join(DEVICE_FILE))?;
    fs::write(source.join(DEVICE_FILE), device_bytes("Changed"))?;
    migrate_default_sync_device(directory.path(), &stable)?;
    assert!(!destination.join(DEVICE_FILE).exists());
    Ok(())
}

#[test]
fn preserves_a_valid_existing_destination() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source = source_dir(directory.path());
    let stable = stable_dir(directory.path());
    fs::create_dir_all(&source)?;
    fs::create_dir_all(stable.join("sync"))?;
    fs::write(source.join(DEVICE_FILE), device_bytes("Legacy"))?;
    fs::write(stable.join("sync/device.json"), device_bytes("Current"))?;
    migrate_default_sync_device(directory.path(), &stable)?;
    assert_eq!(fs::read(stable.join("sync/device.json"))?, device_bytes("Current"));
    assert!(stable.join("sync").join(COMPLETION_MARKER).exists());
    Ok(())
}

#[test]
fn missing_source_retries_later() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let stable = stable_dir(directory.path());
    migrate_default_sync_device(directory.path(), &stable)?;
    assert!(!stable.join("sync").join(COMPLETION_MARKER).exists());
    let source = source_dir(directory.path());
    fs::create_dir_all(&source)?;
    fs::write(source.join(DEVICE_FILE), device_bytes("Late"))?;
    migrate_default_sync_device(directory.path(), &stable)?;
    assert_eq!(fs::read(stable.join("sync/device.json"))?, device_bytes("Late"));
    Ok(())
}

#[test]
fn rejects_invalid_and_oversized_sources() -> Result<(), Box<dyn std::error::Error>> {
    for bytes in [b"{}".to_vec(), vec![b'a'; MAX_DEVICE_BYTES + 1]] {
        let directory = tempfile::tempdir()?;
        let source = source_dir(directory.path());
        let stable = stable_dir(directory.path());
        fs::create_dir_all(&source)?;
        fs::write(source.join(DEVICE_FILE), bytes)?;
        assert!(migrate_default_sync_device(directory.path(), &stable).is_err());
        assert!(!stable.join("sync").join(COMPLETION_MARKER).exists());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_source_ancestor_and_destination_links() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    for linked_component in 0..SOURCE_COMPONENTS.len() {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("source-target");
        let mut real = target.clone();
        for component in &SOURCE_COMPONENTS[linked_component + 1..] {
            real.push(component);
        }
        fs::create_dir_all(&real)?;
        fs::write(real.join(DEVICE_FILE), device_bytes("Linked"))?;
        let mut link = directory.path().to_path_buf();
        for component in &SOURCE_COMPONENTS[..linked_component] {
            link.push(component);
        }
        fs::create_dir_all(&link)?;
        link.push(SOURCE_COMPONENTS[linked_component]);
        symlink(&target, &link)?;
        assert!(
            migrate_default_sync_device(directory.path(), &stable_dir(directory.path())).is_err()
        );
    }
    let directory = tempfile::tempdir()?;
    let stable = stable_dir(directory.path());
    fs::create_dir_all(&stable)?;
    symlink(directory.path().join("elsewhere"), stable.join("sync"))?;
    assert!(migrate_default_sync_device(directory.path(), &stable).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_linked_destination_files() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    for name in [DEVICE_FILE, COMPLETION_MARKER] {
        let directory = tempfile::tempdir()?;
        let stable = stable_dir(directory.path());
        let destination = stable.join("sync");
        let target = directory.path().join("target");
        fs::create_dir_all(&destination)?;
        fs::write(
            &target,
            if name == DEVICE_FILE { device_bytes("Target") } else { COMPLETION_BYTES.to_vec() },
        )?;
        symlink(&target, destination.join(name))?;
        assert!(migrate_default_sync_device(directory.path(), &stable).is_err());
    }
    Ok(())
}

#[test]
fn rejects_hardlinked_source_and_destination() -> Result<(), Box<dyn std::error::Error>> {
    for destination_link in [false, true] {
        let directory = tempfile::tempdir()?;
        let source = source_dir(directory.path());
        let stable = stable_dir(directory.path());
        let target = directory.path().join("target.json");
        fs::create_dir_all(&source)?;
        fs::write(&target, device_bytes("Target"))?;
        if destination_link {
            fs::create_dir_all(stable.join("sync"))?;
            fs::hard_link(&target, stable.join("sync/device.json"))?;
        } else {
            fs::hard_link(&target, source.join(DEVICE_FILE))?;
        }
        assert!(migrate_default_sync_device(directory.path(), &stable).is_err());
    }
    Ok(())
}

fn source_dir(root: &Path) -> std::path::PathBuf {
    root.join("default/servo/sync")
}

fn stable_dir(root: &Path) -> std::path::PathBuf {
    root.join("profile_stable/servo")
}

fn device_bytes(name: &str) -> Vec<u8> {
    format!(
        r#"{{"device_id":"ely-legacy-device","public_key":"{PUBLIC_KEY}","device_name":"{name}","platform":"macos"}}"#,
    )
    .into_bytes()
}
