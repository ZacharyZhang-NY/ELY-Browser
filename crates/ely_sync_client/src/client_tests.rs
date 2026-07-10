use std::{
    error::Error,
    io::{Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
};

use crate::{
    AccountKey, ApiClientConfig, BearerToken, SnapshotCryptoContext, SnapshotDownloadResult,
    SnapshotHeadRef, SnapshotPayload, SnapshotUploadRequest, SnapshotUploadResult, SyncApiClient,
};

type TestServer = JoinHandle<std::io::Result<()>>;

#[test]
fn authenticated_user_id_uses_the_read_only_device_list() -> Result<(), Box<dyn Error>> {
    let (base_url, server) = spawn_authenticated_server(
        "GET /api/devices HTTP/1.1\r\n",
        "200 OK",
        r#"{"version":1,"user_id":"user-01","devices":[]}"#,
    )?;
    let client = SyncApiClient::new(
        ApiClientConfig::custom(base_url, "auto"),
        BearerToken::new("a".repeat(64))?,
    )?;

    assert_eq!(client.authenticated_user_id()?, "user-01");
    join_server(server)?;

    for body in [
        r#"{"version":2,"user_id":"user-01","devices":[]}"#,
        r#"{"version":1,"user_id":"x","devices":[]}"#,
    ] {
        let (base_url, server) =
            spawn_authenticated_server("GET /api/devices HTTP/1.1\r\n", "200 OK", body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;
        assert!(matches!(
            client.authenticated_user_id(),
            Err(crate::SyncClientError::DeviceTrust { .. })
        ));
        join_server(server)?;
    }
    Ok(())
}

#[test]
fn sign_out_posts_the_bearer_and_validates_success() -> Result<(), Box<dyn Error>> {
    let (base_url, server) = spawn_logout_server("200 OK", r#"{"version":1,"signed_out":true}"#)?;
    let client = SyncApiClient::new(
        ApiClientConfig::custom(base_url, "auto"),
        BearerToken::new("a".repeat(64))?,
    )?;

    client.sign_out()?;

    join_server(server)
}

#[test]
fn sign_out_accepts_already_ended_sessions() -> Result<(), Box<dyn Error>> {
    for code in ["session_not_found", "session_expired"] {
        let body = format!(r#"{{"error":"{code}"}}"#);
        let (base_url, server) = spawn_logout_server("401 Unauthorized", &body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;

        client.sign_out()?;
        join_server(server)?;
    }
    Ok(())
}

#[test]
fn sign_out_preserves_retryable_failures() -> Result<(), Box<dyn Error>> {
    let failures = [
        ("401 Unauthorized", r#"{"error":"authorization_invalid"}"#, 401),
        ("401 Unauthorized", r#"{"error":"session_not_found","extra":true}"#, 401),
        ("403 Forbidden", r#"{"error":"session_not_found"}"#, 403),
        ("500 Internal Server Error", r#"{"error":"session_logout_failed"}"#, 500),
    ];
    for (status_line, body, expected_status) in failures {
        let (base_url, server) = spawn_logout_server(status_line, body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;

        let error = match client.sign_out() {
            Ok(()) => return Err("logout failure was treated as success".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            crate::SyncClientError::HttpStatus { status, .. } if status == expected_status
        ));
        join_server(server)?;
    }

    for body in [r#"{"version":1,"signed_out":false}"#, r#"{"version":2,"signed_out":true}"#] {
        let (base_url, server) = spawn_logout_server("200 OK", body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;
        assert!(matches!(client.sign_out(), Err(crate::SyncClientError::SessionLogoutInvalid)));
        join_server(server)?;
    }

    for body in ["{", r#"{"version":1,"signed_out":true,"extra":true}"#, r#"{"version":1}"#] {
        let (base_url, server) = spawn_logout_server("200 OK", body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;
        assert!(matches!(client.sign_out(), Err(crate::SyncClientError::Json { .. })));
        join_server(server)?;
    }
    Ok(())
}

#[test]
fn runtime_requests_classify_only_strict_terminal_sessions() -> Result<(), Box<dyn Error>> {
    for code in ["session_not_found", "session_expired"] {
        let body = format!(r#"{{"error":"{code}"}}"#);
        let (base_url, server) =
            spawn_authenticated_server("GET /api/devices HTTP/1.1\r\n", "401 Unauthorized", &body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;

        assert!(matches!(client.list_devices(), Err(crate::SyncClientError::SessionEnded)));
        join_server(server)?;
    }

    for (status_line, body, expected_status) in [
        ("401 Unauthorized", r#"{"error":"authorization_missing"}"#, 401),
        ("401 Unauthorized", r#"{"error":"authorization_invalid"}"#, 401),
        ("401 Unauthorized", r#"{"error":"unknown"}"#, 401),
        ("401 Unauthorized", r#"{"error":"session_not_found","extra":true}"#, 401),
        ("401 Unauthorized", "{", 401),
        ("403 Forbidden", r#"{"error":"session_not_found"}"#, 403),
        ("500 Internal Server Error", r#"{"error":"session_expired"}"#, 500),
    ] {
        let (base_url, server) =
            spawn_authenticated_server("GET /api/devices HTTP/1.1\r\n", status_line, body)?;
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;

        assert!(matches!(
            client.list_devices(),
            Err(crate::SyncClientError::HttpStatus { status, .. }) if status == expected_status
        ));
        join_server(server)?;
    }
    Ok(())
}

#[test]
fn upload_parses_structured_snapshot_head_conflict() -> Result<(), Box<dyn Error>> {
    let (base_url, server) = spawn_conflict_server()?;
    let client = SyncApiClient::new(
        ApiClientConfig::custom(base_url, "auto"),
        BearerToken::new("a".repeat(64))?,
    )?;
    let key = AccountKey::from_bytes([31; 32]);
    let context = SnapshotCryptoContext {
        user_id: "user-01",
        vault_generation: 1,
        snapshot_id: "snapshot-local",
        schema_rev: 1,
        logical_clock: 8,
        device_id: "device-local",
        head_revision: 1,
        base_head: None,
    };
    let encrypted = key.encrypt(&context, b"local snapshot")?;
    let payload = SnapshotPayload::new(encrypted.bytes().to_vec())?;
    let request = SnapshotUploadRequest::new("auto", &context, None, &encrypted, &payload)?;

    let SnapshotUploadResult::Conflict(conflict) = client.upload_snapshot(&request)? else {
        return Err("snapshot upload conflict was not preserved".into());
    };

    assert_eq!(conflict.current_head.ok_or("missing conflict head")?.head_revision, 7);
    join_server(server)
}

#[test]
fn download_parses_structured_snapshot_head_conflict() -> Result<(), Box<dyn Error>> {
    let (base_url, server) = spawn_conflict_server()?;
    let client = SyncApiClient::new(
        ApiClientConfig::custom(base_url, "auto"),
        BearerToken::new("a".repeat(64))?,
    )?;
    let requested = SnapshotHeadRef::new(6, "snapshot-old", "cd".repeat(32))?;

    let SnapshotDownloadResult::Conflict(conflict) = client.download_snapshot(&requested)? else {
        return Err("snapshot download conflict was not preserved".into());
    };

    assert_eq!(conflict.current_head.ok_or("missing conflict head")?.snapshot_id, "snapshot-new");
    join_server(server)
}

fn spawn_conflict_server() -> Result<(String, TestServer), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let body = serde_json::json!({
        "version": 1,
        "error": "sync_snapshot_head_conflict",
        "current_head": {
            "head_revision": 7,
            "base_head": {
                "revision": 6,
                "snapshot_id": "snapshot-old",
                "payload_hash": "cd".repeat(32)
            },
            "snapshot_id": "snapshot-new",
            "payload_hash": "ab".repeat(32),
            "encryption_version": 2,
            "vault_generation": 1,
            "key_id": "ef".repeat(32),
            "content_hash": "12".repeat(32),
            "logical_clock": 9,
            "device_id": "device-remote",
            "size_bytes": 256,
            "created_at": 1
        }
    })
    .to_string();
    let server = thread::spawn(move || -> std::io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        read_complete_request(&mut stream)?;
        let response = format!(
            "HTTP/1.1 409 Conflict\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes())?;
        stream.flush()
    });
    Ok((format!("http://{address}"), server))
}

/// Reads headers plus the full `Content-Length` body. Responding before the
/// client finishes writing resets the connection and makes tests flaky.
fn read_complete_request(stream: &mut std::net::TcpStream) -> std::io::Result<Vec<u8>> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end = loop {
        if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 || request.len() + read > 64 * 1024 {
            return Err(std::io::Error::other("request headers are incomplete"));
        }
        request.extend_from_slice(&chunk[..read]);
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let content_length = headers
        .split("\r\n")
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().ok())?
        })
        .unwrap_or(0_usize);
    while request.len() < header_end + content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(std::io::Error::other("request body is incomplete"));
        }
        request.extend_from_slice(&chunk[..read]);
    }
    Ok(request)
}

#[test]
fn download_parses_snapshot_bodies_above_ureq_default_cap() -> Result<(), Box<dyn Error>> {
    let data_base64 = "A".repeat(11 * 1024 * 1024);
    let (base_url, server) = spawn_snapshot_download_server(data_base64.clone())?;
    let client = SyncApiClient::new(
        ApiClientConfig::custom(base_url, "auto"),
        BearerToken::new("a".repeat(64))?,
    )?;
    let requested = SnapshotHeadRef::new(7, "snapshot-big", "ab".repeat(32))?;

    let SnapshotDownloadResult::Downloaded(download) = client.download_snapshot(&requested)? else {
        return Err("snapshot download body was not preserved".into());
    };
    assert_eq!(download.data_base64.len(), data_base64.len());
    join_server(server)
}

#[test]
fn oversized_response_bodies_fail_closed() -> Result<(), Box<dyn Error>> {
    let (base_url, server) = spawn_snapshot_download_server("A".repeat(14 * 1024 * 1024))?;
    let client = SyncApiClient::new(
        ApiClientConfig::custom(base_url, "auto"),
        BearerToken::new("a".repeat(64))?,
    )?;
    let requested = SnapshotHeadRef::new(7, "snapshot-big", "ab".repeat(32))?;

    let result = client.download_snapshot(&requested);
    assert!(matches!(
        &result,
        Err(crate::SyncClientError::HttpStatus { body, .. }) if body.contains("sync wire limit")
    ));
    join_server(server)
}

fn spawn_snapshot_download_server(
    data_base64: String,
) -> Result<(String, TestServer), Box<dyn Error>> {
    let body = format!(
        r#"{{"version":3,"user_id":"user-01","device_id":"device-remote","snapshot":{{"snapshot_id":"snapshot-big","r2_key":"snapshots/user-01","payload_hash":"{hash}","encryption_version":2,"vault_generation":1,"key_id":"{key}","content_hash":"{content}","schema_rev":1,"logical_clock":9,"head_revision":7,"base_head":null,"device_id":"device-remote","size_bytes":256,"created_at":1}},"data_base64":"{data_base64}"}}"#,
        hash = "ab".repeat(32),
        key = "ef".repeat(32),
        content = "12".repeat(32),
    );
    spawn_authenticated_server("GET /api/sync/snapshot?snapshot_id=snapshot-big", "200 OK", &body)
}

fn spawn_logout_server(
    status_line: &'static str,
    body: &str,
) -> Result<(String, TestServer), Box<dyn Error>> {
    spawn_authenticated_server("POST /api/session/logout HTTP/1.1\r\n", status_line, body)
}

fn spawn_authenticated_server(
    expected_request_line: &'static str,
    status_line: &'static str,
    body: &str,
) -> Result<(String, TestServer), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let body = body.to_string();
    let server = thread::spawn(move || -> std::io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut chunk)?;
            if read == 0 || request.len() + read > 8192 {
                return Err(std::io::Error::other("logout request headers are incomplete"));
            }
            request.extend_from_slice(&chunk[..read]);
        }
        let request = String::from_utf8_lossy(&request);
        if !request.starts_with(expected_request_line)
            || !request.contains(&format!("Authorization: Bearer {}\r\n", "a".repeat(64)))
        {
            return Err(std::io::Error::other("logout request contract mismatch"));
        }
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes())?;
        stream.flush()
    });
    Ok((format!("http://{address}"), server))
}

fn join_server(server: TestServer) -> Result<(), Box<dyn Error>> {
    match server.join() {
        Ok(result) => result.map_err(Into::into),
        Err(_) => Err("snapshot conflict server thread panicked".into()),
    }
}
