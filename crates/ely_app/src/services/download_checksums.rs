use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use ely_domain::{DomainError, DownloadChecksum};
use sha2::{Digest, Sha256};
use thiserror::Error;

const CHECKSUM_BUFFER_BYTES: usize = 64 * 1024;

pub struct DownloadChecksumCalculator;

#[derive(Debug, Error)]
pub enum DownloadChecksumError {
    #[error("download file is unavailable: {path}")]
    FileUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("download path is not a file: {path}")]
    NotAFile { path: PathBuf },

    #[error(transparent)]
    Domain(#[from] DomainError),
}

impl DownloadChecksumCalculator {
    pub fn sha256(path: &Path) -> Result<DownloadChecksum, DownloadChecksumError> {
        require_file(path)?;
        let mut file = File::open(path).map_err(|source| {
            DownloadChecksumError::FileUnavailable { path: path.to_path_buf(), source }
        })?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; CHECKSUM_BUFFER_BYTES];

        loop {
            let bytes_read = file.read(&mut buffer).map_err(|source| {
                DownloadChecksumError::FileUnavailable { path: path.to_path_buf(), source }
            })?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        DownloadChecksum::sha256_hex(encode_lower_hex(&hasher.finalize())).map_err(Into::into)
    }
}

fn require_file(path: &Path) -> Result<(), DownloadChecksumError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(DownloadChecksumError::NotAFile { path: path.to_path_buf() }),
        Err(source) => {
            Err(DownloadChecksumError::FileUnavailable { path: path.to_path_buf(), source })
        }
    }
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let byte = *byte;
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{DownloadChecksumCalculator, DownloadChecksumError};

    #[test]
    fn computes_sha256_for_download_file() -> Result<(), Box<dyn Error>> {
        let path = temp_file_path("checksum")?;
        fs::write(&path, [])?;

        let checksum = DownloadChecksumCalculator::sha256(&path)?;
        let value = checksum.value().to_string();
        fs::remove_file(path)?;

        assert_eq!(value, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        Ok(())
    }

    #[test]
    fn rejects_missing_download_file() -> Result<(), Box<dyn Error>> {
        let path = temp_file_path("missing")?;

        let error = match DownloadChecksumCalculator::sha256(&path) {
            Ok(_) => return Err("missing download file should be rejected".into()),
            Err(error) => error,
        };

        assert!(matches!(error, DownloadChecksumError::FileUnavailable { .. }));
        Ok(())
    }

    fn temp_file_path(name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(std::env::temp_dir().join(format!("ely-download-{name}-{nanos}.bin")))
    }
}
