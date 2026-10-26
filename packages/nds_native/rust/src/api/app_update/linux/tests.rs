use super::*;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("nds-app-update-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn policy(origin: &str) -> Policy {
    Policy {
        manifest_url: format!("{origin}/app-release.json"),
        bundle_url: format!("{origin}/bundle.json"),
        issuer: "https://token.actions.githubusercontent.com".into(),
        workflow_identity:
            "https://github.com/example/desktop/.github/workflows/update.yml@refs/heads/main".into(),
        repository_uri: "https://github.com/example/desktop".into(),
        repository_id: "123".into(),
        allowed_origins: vec![origin.into()],
        installed_version: "1.0.0-alpha.1".into(),
        approved_sources: vec![Source {
            role: "desktop".into(),
            repository: "https://github.com/example/desktop".into(),
            commit: "a".repeat(40),
        }],
    }
}

#[test]
fn downloaded_metadata_cannot_select_an_origin_or_transport_policy() {
    let p = policy("https://example.invalid");
    assert!(p.validate().is_ok());
    assert!(p.url("https://example.invalid/payload").is_ok());
    for url in [
        "https://other.invalid/payload",
        "https://user:secret@example.invalid/payload",
        "https://example.invalid/payload#fragment",
        "file:///tmp/payload",
        "http://example.invalid/payload",
    ] {
        assert!(p.url(url).is_err(), "{url}");
    }
    let local = policy("http://127.0.0.1:12345");
    assert_eq!(
        local.url("http://127.0.0.1:12345/payload").is_ok(),
        cfg!(feature = "development-http")
    );
    assert!(local.url("http://127.1:12345/payload").is_err());
    assert!(local.url("http://localhost:12345/payload").is_err());
    assert!(policy("http://127.0.0.1:80").validate().is_err());
}

#[test]
fn actual_lock_and_partial_cleanup_preserve_unrelated_files_and_symlinks() {
    let directory = Directory::new();
    let root = &directory.0;
    let first = storage_lock(root).unwrap();
    assert!(matches!(storage_lock(root), Err(Error::Busy)));
    drop(first);
    let _lock = storage_lock(root).unwrap();
    let work = root.join(format!("work-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&work).unwrap();
    write_new(&work.join("payload"), b"interrupted bytes").unwrap();
    write_new(&root.join("unrelated"), b"preserve").unwrap();
    clean_partial(root).unwrap();
    assert!(!work.exists());
    assert_eq!(fs::read(root.join("unrelated")).unwrap(), b"preserve");
    std::os::unix::fs::symlink(root.join("unrelated"), root.join("current")).unwrap();
    assert!(matches!(retain_current(root), Err(Error::Storage)));
    assert_eq!(fs::read(root.join("unrelated")).unwrap(), b"preserve");
}

#[test]
fn committed_candidate_retention_has_one_current_and_refuses_unknown_layouts() {
    let directory = Directory::new();
    let root = &directory.0;
    let current = "a".repeat(64);
    let old_manifest = b"retention fixture metadata";
    let old = digest(old_manifest);
    fs::create_dir(root.join(&current)).unwrap();
    fs::create_dir(root.join(&old)).unwrap();
    write_new(&root.join(&old).join("app-release.json"), old_manifest).unwrap();
    write_new(&root.join(&old).join("bundle.json"), b"previous bundle").unwrap();
    write_new(&root.join(&old).join("payload"), b"previous bytes").unwrap();
    write_new(&root.join("current"), current.as_bytes()).unwrap();
    retain_current(root).unwrap();
    assert!(!root.join(&old).exists());
    assert!(root.join(&current).exists());
    let unknown = root.join("b".repeat(64));
    fs::create_dir(&unknown).unwrap();
    write_new(&unknown.join("personal-data"), b"preserve").unwrap();
    assert!(matches!(retain_current(root), Err(Error::Storage)));
    assert_eq!(
        fs::read(unknown.join("personal-data")).unwrap(),
        b"preserve"
    );
}

/// Real issued NDS manifest and DSSE bundle, never a successful verifier stub.
/// Its independently supplied assembly policy belongs outside the download.
#[test]
#[ignore = "requires the owner-qualified NDS publisher fixture and assembly policy"]
fn authentic_nds_manifest_and_all_document_tampering_use_the_real_verifier() {
    assert!(
        domain::hex(crate::provenance::source_commit(), 40),
        "qualification requires the embedded exact source identity"
    );
    assert_eq!(std::env::var("NDS_ISOLATED_PROVIDER").as_deref(), Ok("1"));
    let root = PathBuf::from(
        std::env::var_os("NDS_APP_UPDATE_FIXTURE").expect("explicit owner-qualified fixture"),
    );
    assert!(root.is_absolute());
    let policy: Policy = serde_json::from_slice(
        &read_bounded(&root.join("assembly-policy.json"), MANIFEST_BYTES).unwrap(),
    )
    .unwrap();
    policy.validate().unwrap();
    let manifest = read_bounded(&root.join("app-release.json"), MANIFEST_BYTES).unwrap();
    let bundle = read_bounded(&root.join("bundle.json"), BUNDLE_BYTES).unwrap();
    let candidate = verify(manifest.clone(), bundle.clone(), &policy).unwrap();
    assert_eq!(candidate.release.data, metadata());
    let mut edited: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    for field in [
        "version", "platform", "channel", "sources", "data", "artifact",
    ] {
        let original = edited[field].clone();
        edited[field] = serde_json::Value::Null;
        assert!(matches!(
            verify(
                serde_json::to_vec(&edited).unwrap(),
                bundle.clone(),
                &policy
            ),
            Err(Error::Signature)
        ));
        edited[field] = original;
    }
    for mutation in 0..4 {
        let mut wrong = policy.clone();
        match mutation {
            0 => wrong.issuer = "https://issuer.invalid".into(),
            1 => wrong.workflow_identity.push_str("-wrong"),
            2 => wrong.repository_id.push('0'),
            _ => wrong.repository_uri.push_str("-wrong"),
        }
        assert!(matches!(
            verify(manifest.clone(), bundle.clone(), &wrong),
            Err(Error::Signature)
        ));
    }
    let mut wrong_graph = policy.clone();
    wrong_graph.approved_sources[0].commit = "f".repeat(40);
    assert!(matches!(
        verify(manifest, bundle, &wrong_graph),
        Err(Error::Source)
    ));
}

#[cfg(feature = "development-http")]
fn fixture() -> (PathBuf, Policy) {
    assert!(
        domain::hex(crate::provenance::source_commit(), 40),
        "qualification requires the embedded exact source identity"
    );
    assert_eq!(std::env::var("NDS_ISOLATED_PROVIDER").as_deref(), Ok("1"));
    let root = PathBuf::from(
        std::env::var_os("NDS_APP_UPDATE_FIXTURE").expect("explicit signed NDS fixture"),
    );
    assert!(root.is_absolute());
    let policy: Policy = serde_json::from_slice(
        &read_bounded(&root.join("assembly-policy.json"), MANIFEST_BYTES).unwrap(),
    )
    .unwrap();
    policy.validate().unwrap();
    (root, policy)
}

#[cfg(feature = "development-http")]
#[tokio::test]
#[ignore = "child of the explicitly invoked real staging qualification"]
async fn qualified_staging_child() {
    assert_eq!(std::env::var("NDS_APP_UPDATE_CHILD").as_deref(), Ok("1"));
    let (_, policy) = fixture();
    let root = PathBuf::from(std::env::var_os("NDS_APP_UPDATE_STAGING_ROOT").unwrap());
    assert!(root.is_absolute());
    let updater = Updater::new(Some(policy), Some(root));
    updater.check().await.unwrap();
    updater.stage().await.unwrap();
}

#[cfg(feature = "development-http")]
#[tokio::test]
#[ignore = "requires an actual NDS-signed manifest, approved policy, owned fixed port and output directory"]
async fn actual_signed_staging_disconnect_cancel_crash_and_offline_restore() {
    use network::SignedServer;
    use std::sync::atomic::Ordering;
    let (fixture, policy) = fixture();
    let case = Directory::new();
    let stage_root = case.0.join("staging");
    let installed = case.0.join("installed-app-canary");
    write_new(&installed, b"existing app fixture").unwrap();
    let server = SignedServer::new(&fixture, &policy);
    let updater = Updater::new(Some(policy.clone()), Some(stage_root.clone()));
    assert!(matches!(
        updater.check().await.unwrap().status,
        Status::Available
    ));
    assert!(matches!(
        updater.stage().await.unwrap().status,
        Status::Staged
    ));
    let pointer = fs::read(stage_root.join("current")).unwrap();
    let payload = stage_root
        .join(std::str::from_utf8(&pointer).unwrap())
        .join("payload");
    let original_payload = fs::read(&payload).unwrap();

    server.mode(1);
    updater.check().await.unwrap();
    assert!(matches!(updater.stage().await, Err(Error::Network)));
    assert_eq!(fs::read(stage_root.join("current")).unwrap(), pointer);
    assert_eq!(fs::read(&payload).unwrap(), original_payload);
    assert_no_partial(&stage_root);

    server.mode(3);
    updater.check().await.unwrap();
    assert!(matches!(updater.stage().await, Err(Error::Manifest)));
    assert_eq!(fs::read(stage_root.join("current")).unwrap(), pointer);
    assert_no_partial(&stage_root);

    server.mode(2);
    updater.check().await.unwrap();
    let download = updater.stage();
    let cancel = async {
        for _ in 0..1000 {
            if partial_has_bytes(&stage_root) {
                updater.cancel();
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("actual partial download did not become observable");
    };
    let (result, ()) = tokio::join!(download, cancel);
    assert!(matches!(result, Err(Error::Cancelled)));
    assert_eq!(fs::read(stage_root.join("current")).unwrap(), pointer);
    assert_no_partial(&stage_root);
    server.mode(0);

    // Kill only this test's newly spawned staging child after actual disk bytes.
    // The parent native app/services and all unrelated processes remain running.
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    server.mode(2);
    let mut child = Child(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "api::app_update::linux::tests::qualified_staging_child",
                "--ignored",
                "--nocapture",
            ])
            .env("NDS_APP_UPDATE_CHILD", "1")
            .env("NDS_APP_UPDATE_STAGING_ROOT", &stage_root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let mut partial = false;
    for _ in 0..1000 {
        if partial_has_bytes(&stage_root) {
            partial = true;
            break;
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "staging child exited before the crash boundary"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(partial, "owned child never wrote partial payload bytes");
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    assert_eq!(fs::read(stage_root.join("current")).unwrap(), pointer);
    assert!(partial_has_bytes(&stage_root));
    server.mode(0);
    drop(server);
    updater.close().await;
    assert!(!updater.busy.load(Ordering::Acquire));
    let restarted = Updater::new(Some(policy), Some(stage_root.clone()));
    let restored = restarted.restore().await.unwrap();
    assert_eq!(restored.status, Status::Staged);
    assert_eq!(fs::read(stage_root.join("current")).unwrap(), pointer);
    assert_eq!(fs::read(payload).unwrap(), original_payload);
    assert_eq!(fs::read(installed).unwrap(), b"existing app fixture");
    assert_no_partial(&stage_root);
    restarted.close().await;

    let evidence = PathBuf::from(
        std::env::var_os("NDS_APP_UPDATE_EVIDENCE").expect("owned private output directory"),
    );
    assert!(
        evidence.is_absolute()
            && !evidence.starts_with(env!("CARGO_MANIFEST_DIR"))
            && !evidence.starts_with(&case.0)
    );
    fs::create_dir_all(&evidence).unwrap();
    write_new(&evidence.join("staging-receipt.json"), &serde_json::to_vec_pretty(&serde_json::json!({
        "scope": "Real NDS publisher DSSE, HTTP, private file staging and crash/restart; opaque qualification payload, not native application distribution",
        "source": option_env!("NDS_SOURCE_COMMIT").unwrap_or("unknown"),
        "manifest_sha256": String::from_utf8(pointer).unwrap(), "payload_sha256": digest(&original_payload),
        "cases": ["verified_commit", "disconnect", "tampered_payload", "mid_body_cancel", "owned_child_crash", "offline_restore", "partial_cleanup"],
        "installed_canary_unchanged": true, "native_keys_and_vault_pending": "UNVERIFIED by this subsystem fixture"
    })).unwrap()).unwrap();
}

#[cfg(feature = "development-http")]
fn partial_has_bytes(root: &Path) -> bool {
    fs::read_dir(root).unwrap().any(|e| {
        let e = e.unwrap();
        e.file_name()
            .to_str()
            .is_some_and(|s| s.starts_with("work-"))
            && fs::metadata(e.path().join("payload")).is_ok_and(|m| m.len() > 0)
    })
}
#[cfg(feature = "development-http")]
fn assert_no_partial(root: &Path) {
    assert!(!fs::read_dir(root).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_str()
            .is_some_and(|s| s.starts_with("work-"))
    }));
}

#[cfg(feature = "development-http")]
mod network {
    use super::*;
    use std::{
        net::{TcpListener, TcpStream},
        sync::Arc,
        thread,
    };

    fn requested_path(socket: &mut TcpStream) -> Option<String> {
        let mut request = [0u8; 8192];
        let mut used = 0;
        loop {
            if used == request.len() {
                return None;
            }
            let n = socket.read(&mut request[used..]).ok()?;
            if n == 0 {
                return None;
            }
            used += n;
            if request[..used].windows(4).any(|s| s == b"\r\n\r\n") {
                break;
            }
        }
        let mut words = std::str::from_utf8(&request[..used])
            .ok()?
            .split_whitespace();
        if words.next()? != "GET" {
            return None;
        }
        Some(words.next()?.to_owned())
    }

    enum Reply {
        Disconnect,
        Slow,
        Oversize,
        Redirect,
    }
    struct Server {
        origin: String,
        stop: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl Server {
        fn new(reply: Reply) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let done = stop.clone();
            let worker = thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while !done.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
                    let Ok((mut socket, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    socket
                        .set_read_timeout(Some(Duration::from_millis(100)))
                        .unwrap();
                    let mut request = [0; 8192];
                    let _ = socket.read(&mut request);
                    match reply {
                        Reply::Disconnect => {
                            let _ = socket.write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\npartial",
                            );
                        }
                        Reply::Oversize => {
                            let _ = socket
                                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n");
                        }
                        Reply::Redirect => {
                            let _ = socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/denied\r\nContent-Length: 0\r\n\r\n");
                        }
                        Reply::Slow => {
                            while !done.load(Ordering::Acquire)
                                && std::time::Instant::now() < deadline
                            {
                                thread::sleep(Duration::from_millis(5));
                            }
                        }
                    }
                    break;
                }
            });
            Self {
                origin: format!("http://{address}"),
                stop,
                worker: Some(worker),
            }
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    pub(super) struct SignedServer {
        stop: Arc<AtomicBool>,
        mode: Arc<std::sync::atomic::AtomicU8>,
        epoch: Arc<std::sync::atomic::AtomicU64>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl SignedServer {
        pub(super) fn new(root: &Path, policy: &Policy) -> Self {
            let manifest = read_bounded(&root.join("app-release.json"), MANIFEST_BYTES).unwrap();
            let bundle = read_bounded(&root.join("bundle.json"), BUNDLE_BYTES).unwrap();
            let candidate = verify(manifest.clone(), bundle.clone(), policy).unwrap();
            let url = policy.url(&candidate.release.artifact.url).unwrap();
            assert_eq!(url.scheme(), "http");
            assert_eq!(url.host_str(), Some("127.0.0.1"));
            let origin = url.origin();
            assert_eq!(policy.url(&policy.manifest_url).unwrap().origin(), origin);
            assert_eq!(policy.url(&policy.bundle_url).unwrap().origin(), origin);
            let payload = read_bounded(&root.join("payload"), 4 * 1024 * 1024).unwrap();
            assert!(payload.len() >= 2);
            assert_eq!(payload.len() as u64, candidate.release.artifact.bytes);
            assert_eq!(digest(&payload), candidate.release.artifact.sha256);
            // Never take over an occupied port or change the signed artifact URL.
            let listener = TcpListener::bind(("127.0.0.1", url.port().unwrap())).unwrap();
            listener.set_nonblocking(true).unwrap();
            let paths = [
                policy.url(&policy.manifest_url).unwrap().path().to_owned(),
                policy.url(&policy.bundle_url).unwrap().path().to_owned(),
                url.path().to_owned(),
            ];
            let stop = Arc::new(AtomicBool::new(false));
            let done = stop.clone();
            let mode = Arc::new(std::sync::atomic::AtomicU8::new(0));
            let behavior = mode.clone();
            let epoch = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let generation = epoch.clone();
            let worker = thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(120);
                while !done.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
                    let Ok((mut socket, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    socket
                        .set_read_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    socket
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = [0; 8192];
                    let Some(path) = requested_path(&mut socket) else {
                        continue;
                    };
                    let body = if path == paths[0] {
                        &manifest
                    } else if path == paths[1] {
                        &bundle
                    } else if path == paths[2] {
                        &payload
                    } else {
                        let _ = socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        continue;
                    };
                    let _ = write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    if path != paths[2] {
                        let _ = socket.write_all(body);
                        continue;
                    }
                    let selected = behavior.load(Ordering::Acquire);
                    let current = generation.load(Ordering::Acquire);
                    match selected {
                        1 => {
                            let _ = socket.write_all(&body[..body.len() / 2]);
                        }
                        2 => {
                            let _ = socket.write_all(&body[..(body.len() / 2).min(CHUNK)]);
                            let _ = socket.flush();
                            while !done.load(Ordering::Acquire)
                                && generation.load(Ordering::Acquire) == current
                                && std::time::Instant::now() < deadline
                            {
                                thread::sleep(Duration::from_millis(5));
                            }
                        }
                        3 => {
                            let mut changed = body.clone();
                            changed[0] ^= 1;
                            let _ = socket.write_all(&changed);
                        }
                        _ => {
                            let _ = socket.write_all(body);
                        }
                    }
                }
            });
            Self {
                stop,
                mode,
                epoch,
                worker: Some(worker),
            }
        }
        pub(super) fn mode(&self, value: u8) {
            self.mode.store(value, Ordering::Release);
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }
    impl Drop for SignedServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.epoch.fetch_add(1, Ordering::AcqRel);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    #[tokio::test]
    async fn actual_disconnect_oversize_and_cross_origin_redirect_are_terminal_failures() {
        for (reply, expected) in [
            (Reply::Disconnect, Error::Network),
            (Reply::Oversize, Error::TooLarge),
            (Reply::Redirect, Error::Network),
        ] {
            let server = Server::new(reply);
            let directory = Directory::new();
            let policy = policy(&server.origin);
            let updater = Updater::new(Some(policy.clone()), Some(directory.0.clone()));
            let operation = updater.begin(Status::Checking, 3).unwrap();
            let result = fetch(
                &client().unwrap(),
                &operation,
                &policy,
                &policy.manifest_url,
                128,
            )
            .await;
            assert_eq!(result, Err(expected));
            assert!(fs::read_dir(&directory.0).unwrap().next().is_none());
        }
    }

    #[tokio::test]
    async fn cancellation_joins_the_actual_pending_request_and_prevents_late_work() {
        let server = Server::new(Reply::Slow);
        let directory = Directory::new();
        let updater = Updater::new(Some(policy(&server.origin)), Some(directory.0.clone()));
        let request = updater.check();
        let cancel = async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            updater.close().await;
        };
        let (result, ()) = tokio::join!(request, cancel);
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!updater.busy.load(Ordering::Acquire));
        assert!(matches!(updater.check().await, Err(Error::Cancelled)));
        assert!(fs::read_dir(&directory.0).unwrap().next().is_none());
    }
}
