use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use ely_domain::UrlText;
use thiserror::Error;
use url::Url;

const DOWNLOAD_BUFFER_BYTES: usize = 64 * 1024;
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(120);
const USER_AGENT: &str = concat!("ELY Browser/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug)]
pub struct HttpDownloadPlan {
    url: UrlText,
    target_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpDownloadResult {
    target_path: PathBuf,
    received_bytes: u64,
    total_bytes: Option<u64>,
}

#[derive(Debug, Error)]
pub enum HttpDownloadError {
    #[error("download URL is invalid: {url}")]
    InvalidUrl { url: String },

    #[error("unsupported download URL scheme: {scheme}")]
    UnsupportedScheme { scheme: String },

    #[error("download target has no parent directory: {path}")]
    MissingTargetDirectory { path: PathBuf },

    #[error("failed to create download directory: {path}: {source}")]
    DirectoryUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to create download file: {path}: {source}")]
    CreateFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("download returned HTTP {status} for {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("download request failed for {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: Box<ureq::Error>,
    },

    #[error("failed to read download stream for {url}: {source}")]
    Read {
        url: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to write download file: {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to finalize download file: {from} -> {to}: {source}")]
    FinalizeFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("download ended at {received_bytes} bytes, expected {total_bytes} bytes for {url}")]
    ContentLengthMismatch { url: String, received_bytes: u64, total_bytes: u64 },
}

impl HttpDownloadPlan {
    #[must_use]
    pub fn new(url: UrlText, target_path: PathBuf) -> Self {
        Self { url, target_path }
    }

    #[must_use]
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }
}

impl HttpDownloadResult {
    #[must_use]
    pub fn received_bytes(&self) -> u64 {
        self.received_bytes
    }
}

pub fn download_to_file(plan: HttpDownloadPlan) -> Result<HttpDownloadResult, HttpDownloadError> {
    validate_download_url(&plan.url)?;
    prepare_target_directory(plan.target_path())?;

    let response = call_download_url(&plan.url)?;
    let total_bytes = content_length(&response);
    let mut reader = response.into_reader();
    let (temporary_path, mut file) = create_temporary_file(plan.target_path())?;
    let stream_result = stream_response_body(&plan.url, &temporary_path, &mut reader, &mut file);
    let received_bytes = match stream_result {
        Ok(received_bytes) => received_bytes,
        Err(error) => {
            _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
    };

    if let Err(error) = validate_content_length(plan.url.as_str(), received_bytes, total_bytes) {
        _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    fs::rename(&temporary_path, plan.target_path()).map_err(|source| {
        HttpDownloadError::FinalizeFile {
            from: temporary_path,
            to: plan.target_path.clone(),
            source,
        }
    })?;

    Ok(HttpDownloadResult { target_path: plan.target_path, received_bytes, total_bytes })
}

fn stream_response_body(
    url: &UrlText,
    target_path: &Path,
    reader: &mut dyn Read,
    file: &mut File,
) -> Result<u64, HttpDownloadError> {
    let mut buffer = [0; DOWNLOAD_BUFFER_BYTES];
    let mut received_bytes = 0;

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|source| HttpDownloadError::Read { url: url.as_str().to_string(), source })?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read]).map_err(|source| HttpDownloadError::Write {
            path: target_path.to_path_buf(),
            source,
        })?;
        received_bytes += bytes_read as u64;
    }

    file.flush()
        .map_err(|source| HttpDownloadError::Write { path: target_path.to_path_buf(), source })?;
    Ok(received_bytes)
}

fn call_download_url(url: &UrlText) -> Result<ureq::Response, HttpDownloadError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(DOWNLOAD_CONNECT_TIMEOUT)
        .timeout_read(DOWNLOAD_READ_TIMEOUT)
        .build();

    match agent
        .get(url.as_str())
        .set("User-Agent", USER_AGENT)
        .set("Accept-Encoding", "identity")
        .call()
    {
        Ok(response) => Ok(response),
        Err(ureq::Error::Status(status, _)) => {
            Err(HttpDownloadError::HttpStatus { url: url.as_str().to_string(), status })
        }
        Err(source) => Err(HttpDownloadError::Request {
            url: url.as_str().to_string(),
            source: Box::new(source),
        }),
    }
}

fn validate_download_url(url: &UrlText) -> Result<(), HttpDownloadError> {
    let parsed = Url::parse(url.as_str())
        .map_err(|_| HttpDownloadError::InvalidUrl { url: url.as_str().to_string() })?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        scheme => Err(HttpDownloadError::UnsupportedScheme { scheme: scheme.to_string() }),
    }
}

fn prepare_target_directory(target_path: &Path) -> Result<(), HttpDownloadError> {
    let Some(directory) = target_path.parent() else {
        return Err(HttpDownloadError::MissingTargetDirectory { path: target_path.to_path_buf() });
    };

    fs::create_dir_all(directory).map_err(|source| HttpDownloadError::DirectoryUnavailable {
        path: directory.to_path_buf(),
        source,
    })
}

fn create_temporary_file(target_path: &Path) -> Result<(PathBuf, File), HttpDownloadError> {
    let file_name =
        target_path.file_name().map(|value| value.to_string_lossy()).unwrap_or_default();
    let prefix = if file_name.is_empty() { "download".into() } else { file_name };

    for attempt in 0..1_000 {
        let temporary_name = if attempt == 0 {
            format!(".{prefix}.elydownload")
        } else {
            format!(".{prefix}.elydownload.{attempt}")
        };
        let temporary_path = target_path.with_file_name(temporary_name);
        match OpenOptions::new().create_new(true).write(true).open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(HttpDownloadError::CreateFile { path: temporary_path, source });
            }
        }
    }

    Err(HttpDownloadError::CreateFile {
        path: target_path.with_file_name(format!(".{prefix}.elydownload")),
        source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary download path exhausted"),
    })
}

fn content_length(response: &ureq::Response) -> Option<u64> {
    response.header("Content-Length").and_then(|value| value.parse().ok())
}

fn validate_content_length(
    url: &str,
    received_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<(), HttpDownloadError> {
    if let Some(total_bytes) = total_bytes
        && received_bytes != total_bytes
    {
        return Err(HttpDownloadError::ContentLengthMismatch {
            url: url.to_string(),
            received_bytes,
            total_bytes,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        io::{Read, Write},
        net::TcpListener,
        path::PathBuf,
        thread::{self, JoinHandle},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use ely_domain::UrlText;

    use super::{HttpDownloadError, HttpDownloadPlan, download_to_file};

    type TestServerHandle = JoinHandle<Result<(), std::io::Error>>;
    type SpawnedHttpServer = (String, TestServerHandle);

    #[test]
    fn downloads_local_http_response_to_file() -> Result<(), Box<dyn Error>> {
        let body = b"ELY download body".to_vec();
        let (url, server) = spawn_http_server(body.clone(), 200)?;
        let target_path = temp_file_path("body")?;

        let result =
            download_to_file(HttpDownloadPlan::new(UrlText::parse(url)?, target_path.clone()))?;

        assert_eq!(result.target_path, target_path);
        assert_eq!(result.received_bytes(), body.len() as u64);
        assert_eq!(result.total_bytes, Some(body.len() as u64));
        assert_eq!(fs::read(&target_path)?, body);
        fs::remove_file(target_path)?;
        join_server(server)?;
        Ok(())
    }

    #[test]
    fn rejects_internal_scheme_before_writing_file() -> Result<(), Box<dyn Error>> {
        let target_path = temp_file_path("internal")?;

        let error = match download_to_file(HttpDownloadPlan::new(
            UrlText::parse("ely://downloads")?,
            target_path.clone(),
        )) {
            Ok(_) => return Err("internal scheme download succeeded".into()),
            Err(error) => error,
        };

        assert!(
            matches!(error, HttpDownloadError::UnsupportedScheme { scheme } if scheme == "ely")
        );
        assert!(!target_path.exists());
        Ok(())
    }

    #[test]
    fn reports_http_status_errors() -> Result<(), Box<dyn Error>> {
        let (url, server) = spawn_http_server(Vec::new(), 404)?;
        let target_path = temp_file_path("status")?;

        let error = match download_to_file(HttpDownloadPlan::new(
            UrlText::parse(url.clone())?,
            target_path.clone(),
        )) {
            Ok(_) => return Err("HTTP error download succeeded".into()),
            Err(error) => error,
        };

        assert!(matches!(error, HttpDownloadError::HttpStatus { status: 404, .. }));
        assert!(!target_path.exists());
        join_server(server)?;
        Ok(())
    }

    fn spawn_http_server(body: Vec<u8>, status: u16) -> Result<SpawnedHttpServer, Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = thread::spawn(move || -> Result<(), std::io::Error> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut request_buffer = [0; 1024];
            _ = stream.read(&mut request_buffer)?;

            let status_text = if status == 200 { "OK" } else { "Not Found" };
            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes())?;
            stream.write_all(&body)?;
            stream.flush()
        });

        Ok((format!("http://{address}/download.bin"), handle))
    }

    fn join_server(handle: TestServerHandle) -> Result<(), Box<dyn Error>> {
        match handle.join() {
            Ok(result) => result.map_err(Into::into),
            Err(_) => Err("HTTP server thread panicked".into()),
        }
    }

    fn temp_file_path(name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(std::env::temp_dir().join(format!("ely-http-download-{name}-{nanos}.bin")))
    }
}
