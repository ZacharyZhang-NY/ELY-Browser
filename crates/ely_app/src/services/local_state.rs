//! On-disk browser state for one standard profile:
//! `<profile-data-root>/<profile-id>/local-state.json`, published
//! atomically so a crash mid-write can never truncate the previous state.

use std::{
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};

use ely_domain::ProfileId;

const LOCAL_STATE_FILE: &str = "local-state.json";

pub(crate) fn local_state_path(profile_data_root: &Path, profile_id: &ProfileId) -> PathBuf {
    profile_data_root.join(profile_id.as_str()).join(LOCAL_STATE_FILE)
}

pub(crate) fn save_local_state(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("local state path has no parent directory"))?;
    fs::create_dir_all(directory)?;
    let temporary = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)
}

/// Missing file is a normal first launch. Read or parse failures stay with
/// the caller so a broken restore is loud, quarantined, and recoverable.
pub(crate) fn load_local_state(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn quarantine_local_state(path: &Path) -> io::Result<PathBuf> {
    let quarantined = path.with_extension("json.corrupt");
    fs::rename(path, &quarantined)?;
    Ok(quarantined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let path = local_state_path(directory.path(), &profile_id);

        assert_eq!(load_local_state(&path)?, None);
        save_local_state(&path, b"{\"local_rev\":1}")?;
        assert_eq!(load_local_state(&path)?, Some(b"{\"local_rev\":1}".to_vec()));

        save_local_state(&path, b"{\"local_rev\":2}")?;
        assert_eq!(load_local_state(&path)?, Some(b"{\"local_rev\":2}".to_vec()));
        Ok(())
    }

    #[test]
    fn quarantine_moves_the_corrupt_file_aside() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let path = local_state_path(directory.path(), &profile_id);
        save_local_state(&path, b"broken")?;

        let quarantined = quarantine_local_state(&path)?;

        assert_eq!(load_local_state(&path)?, None);
        assert_eq!(std::fs::read(quarantined)?, b"broken");
        Ok(())
    }
}
