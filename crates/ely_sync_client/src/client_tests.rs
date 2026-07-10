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
        let mut request = [0_u8; 16 * 1024];
        let _ = stream.read(&mut request)?;
        let response = format!(
            "HTTP/1.1 409 Conflict\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
