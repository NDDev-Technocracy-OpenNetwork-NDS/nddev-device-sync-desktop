use super::{AppUpdateError as Error, AppUpdateStatus as Status, AppUpdateView as View};
use nddev_device_sync_domain::app_update::{
    self as domain, DataVersions, Installed, PublisherSource, Release, Source,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sigstore_trust_root::{SIGSTORE_PRODUCTION_TRUSTED_ROOT, TrustedRoot};
use sigstore_types::{Bundle, SignatureContent, Statement};
use sigstore_verify::{VerificationPolicy, Verifier};
use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

const MANIFEST_BYTES: usize = 65_536;
const BUNDLE_BYTES: usize = 262_144;
const CHUNK: usize = 65_536;
const OWNER_FILE: &str = ".nds-app-update-owner";
const OWNER_BYTES: &[u8] = b"NDS application staging namespace v1\n";

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    manifest_url: String,
    bundle_url: String,
    issuer: String,
    workflow_identity: String,
    repository_uri: String,
    repository_id: String,
    allowed_origins: Vec<String>,
    approved_sources: Vec<Source>,
    installed_version: String,
}

impl Policy {
    fn validate(&self) -> Result<(), Error> {
        if self.issuer != "https://token.actions.githubusercontent.com"
            || !domain::repository(&self.repository_uri)
            || self.repository_id.is_empty()
            || self.repository_id.len() > 20
            || !self.repository_id.bytes().all(|b| b.is_ascii_digit())
            || !self
                .workflow_identity
                .starts_with(&format!("{}/.github/workflows/", self.repository_uri))
            || !self.workflow_identity.ends_with("@refs/heads/main")
            || self.workflow_identity.len() > 512
            || domain::candidate(&self.installed_version).is_err()
            || self.approved_sources.is_empty()
            || self.approved_sources.len() > 64
            || self.allowed_origins.is_empty()
            || self.allowed_origins.len() > 16
        {
            return Err(Error::NotConfigured);
        }
        self.url(&self.manifest_url)?;
        self.url(&self.bundle_url)?;
        Ok(())
    }

    fn url(&self, value: &str) -> Result<Url, Error> {
        let url = Url::parse(value).map_err(|_| Error::Network)?;
        let secure = url.scheme() == "https";
        let development = cfg!(feature = "development-http")
            && url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some_and(|p| {
                value
                    .split_once("://")
                    .and_then(|(_, tail)| tail.split('/').next())
                    == Some(format!("127.0.0.1:{p}").as_str())
            })
            && url.port().is_some_and(|p| p >= 1024);
        if value.len() > 2048
            || value.bytes().any(|b| b.is_ascii_control() || b == b' ')
            || !(secure || development)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !self
                .allowed_origins
                .iter()
                .any(|origin| *origin == url.origin().ascii_serialization())
        {
            return Err(Error::Network);
        }
        Ok(url)
    }
}

#[derive(Clone)]
struct Candidate {
    release: Release,
    manifest: Vec<u8>,
    bundle: Vec<u8>,
    identity: String,
}

struct State {
    view: View,
    candidate: Option<Candidate>,
    cancellation: CancellationToken,
}

pub(super) struct Updater {
    policy: Option<Policy>,
    root: Option<PathBuf>,
    state: Mutex<State>,
    busy: AtomicBool,
    closed: AtomicBool,
    done: Notify,
    // Cancellation and committing the current pointer have one linearization lock.
    commit: Mutex<()>,
}

struct Operation<'a> {
    owner: &'a Updater,
    cancel: CancellationToken,
    deadline: tokio::time::Instant,
}
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.owner.busy.store(false, Ordering::Release);
        self.owner.done.notify_waiters();
    }
}
impl Operation<'_> {
    fn live(&self) -> Result<(), Error> {
        if self.cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else if tokio::time::Instant::now() >= self.deadline {
            Err(Error::Timeout)
        } else {
            Ok(())
        }
    }
    async fn wait<T>(&self, future: impl std::future::Future<Output = T>) -> Result<T, Error> {
        self.live()?;
        tokio::select! {
            biased;
            _ = self.cancel.cancelled() => Err(Error::Cancelled),
            _ = tokio::time::sleep_until(self.deadline) => Err(Error::Timeout),
            value = future => Ok(value),
        }
    }
}

fn metadata() -> DataVersions {
    DataVersions {
        vault_journal: nddev_device_sync_vault::journal_schema_version(),
        sync_wire: nddev_device_sync_accounts::sync_wire_schema_version(),
        accounts_fence: nddev_device_sync_accounts::authorization_fence_schema_version(),
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn domain_error(error: domain::Error) -> Error {
    match error {
        domain::Error::Compatibility => Error::Compatibility,
        domain::Error::Source => Error::Source,
        domain::Error::NotNewer => Error::NotNewer,
        _ => Error::Manifest,
    }
}

fn verify(manifest: Vec<u8>, bytes: Vec<u8>, policy: &Policy) -> Result<Candidate, Error> {
    if manifest.len() > MANIFEST_BYTES || bytes.len() > BUNDLE_BYTES {
        return Err(Error::TooLarge);
    }
    let bundle = Bundle::from_json(std::str::from_utf8(&bytes).map_err(|_| Error::Signature)?)
        .map_err(|_| Error::Signature)?;
    let SignatureContent::DsseEnvelope(envelope) = &bundle.content else {
        return Err(Error::Signature);
    };
    if envelope.payload_type != "application/vnd.in-toto+json"
        || bundle.verification_material.tlog_entries.len() != 1
    {
        return Err(Error::Signature);
    }
    let statement: Statement =
        serde_json::from_slice(envelope.payload.as_bytes()).map_err(|_| Error::Signature)?;
    if statement.type_ != "https://in-toto.io/Statement/v1"
        || statement.predicate_type != "https://slsa.dev/provenance/v1"
        || statement.subject.len() != 1
        || statement.subject[0].name != "app-release.json"
        || statement.subject[0]
            .digest
            .sha256
            .as_ref()
            .map(|h| h.to_hex())
            != Some(digest(&manifest))
    {
        return Err(Error::Signature);
    }
    let root =
        TrustedRoot::from_json(SIGSTORE_PRODUCTION_TRUSTED_ROOT).map_err(|_| Error::Signature)?;
    let verified = Verifier::new(&root)
        .map_err(|_| Error::Signature)?
        .verify_reader(
            Cursor::new(&manifest),
            &bundle,
            &VerificationPolicy::new(policy.workflow_identity.as_str(), policy.issuer.as_str()),
        )
        .map_err(|_| Error::Signature)?;
    if !(verified.certificate_verified()
        && verified.sct_verified()
        && verified.tlog_verified()
        && verified.identity_policy_checked())
    {
        return Err(Error::Signature);
    }
    let claims = &verified.certificate().ok_or(Error::Signature)?.ci_claims;
    if claims.source_repository_uri.as_deref() != Some(policy.repository_uri.as_str())
        || claims.source_repository_identifier.as_deref() != Some(policy.repository_id.as_str())
        || claims.source_repository_ref.as_deref() != Some("refs/heads/main")
    {
        return Err(Error::Signature);
    }
    let source = claims
        .source_repository_digest
        .as_deref()
        .ok_or(Error::Source)?;
    let release: Release = serde_json::from_slice(&manifest).map_err(|_| Error::Manifest)?;
    release
        .eligible(
            &Installed {
                version: &policy.installed_version,
                architecture: std::env::consts::ARCH,
                data: Some(metadata()),
                approved_sources: &policy.approved_sources,
                application_repository: &policy.repository_uri,
            },
            &PublisherSource {
                repository: &policy.repository_uri,
                commit: source,
            },
        )
        .map_err(domain_error)?;
    policy.url(&release.artifact.url)?;
    Ok(Candidate {
        identity: digest(&manifest),
        release,
        manifest,
        bundle: bytes,
    })
}

impl Updater {
    pub(super) fn embedded() -> Self {
        let policy = option_env!("NDS_APP_UPDATE_POLICY")
            .filter(|s| s.len() <= MANIFEST_BYTES)
            .and_then(|s| serde_json::from_str::<Policy>(s).ok())
            .filter(|p| {
                p.validate().is_ok() && domain::hex(crate::provenance::source_commit(), 40)
            });
        let home = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
        Self::new(policy, home.map(|p| p.join("nds/app-updates")))
    }
    fn new(policy: Option<Policy>, root: Option<PathBuf>) -> Self {
        let configured = policy.is_some() && root.as_ref().is_some_and(|p| p.is_absolute());
        Self {
            policy,
            root,
            state: Mutex::new(State {
                view: View::empty(if configured {
                    Status::Idle
                } else {
                    Status::NotConfigured
                }),
                candidate: None,
                cancellation: CancellationToken::new(),
            }),
            busy: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            done: Notify::new(),
            commit: Mutex::new(()),
        }
    }
    pub(super) fn snapshot(&self) -> View {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .view
            .clone()
    }
    fn begin(&self, status: Status, seconds: u64) -> Result<Operation<'_>, Error> {
        let _commit = self.commit.lock().unwrap_or_else(|e| e.into_inner());
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        if self.policy.is_none() || !self.root.as_ref().is_some_and(|p| p.is_absolute()) {
            return Err(Error::NotConfigured);
        }
        self.busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| Error::Busy)?;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.cancellation = CancellationToken::new();
        state.view.status = status;
        state.view.error = None;
        if status == Status::Checking {
            state.candidate = None;
            state.view = View::empty(status);
        }
        Ok(Operation {
            owner: self,
            cancel: state.cancellation.clone(),
            deadline: tokio::time::Instant::now() + Duration::from_secs(seconds),
        })
    }
    fn finish(
        &self,
        result: Result<Option<Candidate>, Error>,
        status: Status,
    ) -> Result<View, Error> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(candidate) => {
                state.view = View::empty(if candidate.is_some() {
                    status
                } else {
                    Status::Idle
                });
                if let Some(c) = &candidate {
                    state.view.version = c.release.version.clone();
                    state.view.total_bytes = c.release.artifact.bytes;
                    if status == Status::Staged {
                        state.view.downloaded_bytes = state.view.total_bytes;
                    }
                }
                state.candidate = candidate;
                Ok(state.view.clone())
            }
            Err(error) => {
                state.view.status = Status::Failed;
                state.view.error = Some(error);
                Err(error)
            }
        }
    }
    pub(super) fn cancel(&self) {
        let _commit = self.commit.lock().unwrap_or_else(|e| e.into_inner());
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancellation
            .cancel();
    }
    pub(super) async fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.cancel();
        loop {
            let done = self.done.notified();
            if !self.busy.load(Ordering::Acquire) {
                return;
            }
            done.await;
        }
    }

    pub(super) async fn check(&self) -> Result<View, Error> {
        let operation = self.begin(Status::Checking, 60)?;
        let result = async {
            let policy = self.policy.as_ref().ok_or(Error::NotConfigured)?;
            let client = client()?;
            let manifest = fetch(
                &client,
                &operation,
                policy,
                &policy.manifest_url,
                MANIFEST_BYTES,
            )
            .await?;
            let bundle = fetch(
                &client,
                &operation,
                policy,
                &policy.bundle_url,
                BUNDLE_BYTES,
            )
            .await?;
            let policy = policy.clone();
            // Join cryptographic work even if cancellation arrives. It cannot
            // publish a result after the original operation has been cancelled.
            let value = tokio::task::spawn_blocking(move || verify(manifest, bundle, &policy))
                .await
                .map_err(|_| Error::Signature)??;
            operation.live()?;
            Ok(Some(value))
        }
        .await;
        self.finish(result, Status::Available)
    }

    pub(super) async fn restore(&self) -> Result<View, Error> {
        let operation = self.begin(Status::Checking, 120)?;
        let result = async {
            let root = self.root.as_ref().ok_or(Error::NotConfigured)?;
            if absent(root) {
                return Ok(None);
            }
            let _lock = storage_lock(root)?;
            clean_partial(root)?;
            retain_current(root)?;
            let pointer = match read_bounded(&root.join("current"), 64) {
                Ok(p) => p,
                Err(_) if absent(&root.join("current")) => return Ok(None),
                Err(e) => return Err(e),
            };
            let identity = std::str::from_utf8(&pointer)
                .map_err(|_| Error::Storage)?
                .to_owned();
            if !domain::hex(&identity, 64) {
                return Err(Error::Storage);
            }
            let path = root.join(&identity);
            let policy = self.policy.clone().ok_or(Error::NotConfigured)?;
            let cancel = operation.cancel.clone();
            let c = tokio::task::spawn_blocking(move || {
                local_candidate(&path, identity, &policy, &cancel)
            })
            .await
            .map_err(|_| Error::Storage)??;
            operation.live()?;
            Ok(Some(c))
        }
        .await;
        self.finish(result, Status::Staged)
    }

    pub(super) async fn stage(&self) -> Result<View, Error> {
        let operation = self.begin(Status::Downloading, 600)?;
        let result = async {
            let candidate = self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .candidate
                .clone()
                .ok_or(Error::Manifest)?;
            let policy = self.policy.as_ref().ok_or(Error::NotConfigured)?;
            let root = self.root.as_ref().ok_or(Error::NotConfigured)?;
            ensure_root(root)?;
            let _lock = storage_lock(root)?;
            clean_partial(root)?;
            retain_current(root)?;
            let work = root.join(format!("work-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&work).map_err(|_| Error::Storage)?;
            fs::set_permissions(&work, fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::Storage)?;
            let scratch = Scratch(work);
            let mut output = create(&scratch.0.join("payload"))?;
            let mut response = request(
                &client()?,
                &operation,
                policy,
                &candidate.release.artifact.url,
            )
            .await?;
            if response
                .content_length()
                .is_some_and(|n| n != candidate.release.artifact.bytes)
            {
                return Err(Error::TooLarge);
            }
            let mut hash = Sha256::new();
            let mut received = 0u64;
            while let Some(bytes) = operation
                .wait(response.chunk())
                .await?
                .map_err(|_| Error::Network)?
            {
                for chunk in bytes.chunks(CHUNK) {
                    operation.live()?;
                    received = received
                        .checked_add(chunk.len() as u64)
                        .ok_or(Error::TooLarge)?;
                    if received > candidate.release.artifact.bytes {
                        return Err(Error::TooLarge);
                    }
                    output.write_all(chunk).map_err(|_| Error::Storage)?;
                    hash.update(chunk);
                    self.state
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .view
                        .downloaded_bytes = received;
                }
            }
            if received != candidate.release.artifact.bytes
                || format!("{:x}", hash.finalize()) != candidate.release.artifact.sha256
            {
                return Err(Error::Manifest);
            }
            output.sync_all().map_err(|_| Error::Storage)?;
            drop(output);
            write_new(&scratch.0.join("app-release.json"), &candidate.manifest)?;
            write_new(&scratch.0.join("bundle.json"), &candidate.bundle)?;
            File::open(&scratch.0)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::Storage)?;
            let target = root.join(&candidate.identity);
            let existing = !absent(&target);
            if existing {
                let path = target.clone();
                let identity = candidate.identity.clone();
                let policy = policy.clone();
                let cancel = operation.cancel.clone();
                tokio::task::spawn_blocking(move || {
                    local_candidate(&path, identity, &policy, &cancel)
                })
                .await
                .map_err(|_| Error::Storage)??;
            }
            let pointer = root.join(format!("work-{}", uuid::Uuid::new_v4()));
            write_new(&pointer, candidate.identity.as_bytes())?;
            let pointer_cleanup = Scratch(pointer);
            let _commit = self.commit.lock().unwrap_or_else(|e| e.into_inner());
            operation.live()?;
            if !existing {
                fs::rename(&scratch.0, &target).map_err(|_| Error::Storage)?;
            }
            fs::rename(&pointer_cleanup.0, root.join("current")).map_err(|_| Error::Storage)?;
            File::open(root)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::Storage)?;
            drop(_commit);
            retain_current(root)?;
            Ok(Some(candidate))
        }
        .await;
        self.finish(result, Status::Staged)
    }
}

fn client() -> Result<Client, Error> {
    // This feature is reachable before IdentityClient::new, which normally
    // installs the same provider. Preserve any already-selected provider.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .no_proxy()
        .build()
        .map_err(|_| Error::Network)
}
async fn request(
    client: &Client,
    op: &Operation<'_>,
    policy: &Policy,
    value: &str,
) -> Result<reqwest::Response, Error> {
    let mut url = policy.url(value)?;
    for hop in 0..=3 {
        let response = op
            .wait(client.get(url.clone()).send())
            .await?
            .map_err(|_| Error::Network)?;
        if response.status().is_redirection() {
            if hop == 3 {
                return Err(Error::Network);
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or(Error::Network)?
                .to_str()
                .map_err(|_| Error::Network)?;
            url = policy.url(url.join(location).map_err(|_| Error::Network)?.as_str())?;
        } else {
            if response.status() != reqwest::StatusCode::OK
                || response
                    .headers()
                    .contains_key(reqwest::header::CONTENT_ENCODING)
            {
                return Err(Error::Network);
            }
            return Ok(response);
        }
    }
    Err(Error::Network)
}
async fn fetch(
    client: &Client,
    op: &Operation<'_>,
    policy: &Policy,
    value: &str,
    bound: usize,
) -> Result<Vec<u8>, Error> {
    let mut response = request(client, op, policy, value).await?;
    if response.content_length().is_some_and(|n| n > bound as u64) {
        return Err(Error::TooLarge);
    }
    let mut output = Vec::new();
    while let Some(bytes) = op
        .wait(response.chunk())
        .await?
        .map_err(|_| Error::Network)?
    {
        if bytes.len() > bound - output.len() {
            return Err(Error::TooLarge);
        }
        output.extend_from_slice(&bytes);
    }
    Ok(output)
}

fn ensure_root(root: &Path) -> Result<(), Error> {
    if !root.is_absolute() {
        return Err(Error::Storage);
    }
    if absent(root) {
        fs::create_dir_all(root).map_err(|_| Error::Storage)?;
    }
    let metadata = fs::symlink_metadata(root).map_err(|_| Error::Storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::Storage);
    }
    let marker = root.join(OWNER_FILE);
    if absent(&marker) {
        // Claim only a new/empty namespace. A work-shaped name is never proof
        // of ownership; no chmod, lock creation or deletion precedes this gate.
        if fs::read_dir(root)
            .map_err(|_| Error::Storage)?
            .next()
            .is_some()
        {
            return Err(Error::Storage);
        }
        write_new(&marker, OWNER_BYTES)?;
        File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Storage)?;
    }
    owned_root(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|_| Error::Storage)
}
fn owned_root(root: &Path) -> Result<(), Error> {
    if !fs::symlink_metadata(root)
        .map_err(|_| Error::Storage)?
        .is_dir()
        || read_bounded(&root.join(OWNER_FILE), OWNER_BYTES.len()).map_err(|_| Error::Storage)?
            != OWNER_BYTES
    {
        return Err(Error::Storage);
    }
    Ok(())
}
fn storage_lock(root: &Path) -> Result<File, Error> {
    ensure_root(root)?;
    let path = root.join("lock");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() => return Err(Error::Storage),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(Error::Storage),
        _ => {}
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map_err(|_| Error::Storage)?;
    lock.try_lock().map_err(|e| match e {
        std::fs::TryLockError::WouldBlock => Error::Busy,
        _ => Error::Storage,
    })?;
    Ok(lock)
}
fn create(path: &Path) -> Result<File, Error> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| Error::Storage)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = create(path)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Error::Storage)
}
fn read_bounded(path: &Path, bound: usize) -> Result<Vec<u8>, Error> {
    if !fs::symlink_metadata(path)
        .map_err(|_| Error::Storage)?
        .is_file()
    {
        return Err(Error::Storage);
    }
    let mut result = Vec::new();
    File::open(path)
        .map_err(|_| Error::Storage)?
        .take(bound as u64 + 1)
        .read_to_end(&mut result)
        .map_err(|_| Error::Storage)?;
    if result.len() > bound {
        return Err(Error::TooLarge);
    }
    Ok(result)
}
fn absent(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        if self.0.is_dir() {
            let _ = fs::remove_dir_all(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
    }
}
fn clean_partial(root: &Path) -> Result<(), Error> {
    owned_root(root)?;
    let mut count = 0;
    for entry in fs::read_dir(root).map_err(|_| Error::Storage)? {
        count += 1;
        if count > 16 {
            return Err(Error::Storage);
        }
        let entry = entry.map_err(|_| Error::Storage)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::Storage)?;
        if name
            .strip_prefix("work-")
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        {
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| Error::Storage)?;
            if metadata.is_dir() {
                fs::remove_dir_all(entry.path()).map_err(|_| Error::Storage)?;
            } else if metadata.is_file() {
                fs::remove_file(entry.path()).map_err(|_| Error::Storage)?;
            } else {
                return Err(Error::Storage);
            }
        }
    }
    Ok(())
}

/// The update directory is an exclusive, private staging namespace. Keep only
/// its committed candidate; interrupted work is handled by clean_partial.
/// Never traverse or remove an unrecognized layout, symlink or external store.
fn retain_current(root: &Path) -> Result<(), Error> {
    owned_root(root)?;
    let current = match read_bounded(&root.join("current"), 64) {
        Ok(bytes) => {
            let id = String::from_utf8(bytes).map_err(|_| Error::Storage)?;
            if !domain::hex(&id, 64) {
                return Err(Error::Storage);
            }
            Some(id)
        }
        Err(_) if absent(&root.join("current")) => None,
        Err(error) => return Err(error),
    };
    let mut count = 0;
    for entry in fs::read_dir(root).map_err(|_| Error::Storage)? {
        count += 1;
        if count > 16 {
            return Err(Error::Storage);
        }
        let entry = entry.map_err(|_| Error::Storage)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::Storage)?;
        if !domain::hex(name, 64) || current.as_deref() == Some(name) {
            continue;
        }
        if !fs::symlink_metadata(entry.path())
            .map_err(|_| Error::Storage)?
            .is_dir()
        {
            return Err(Error::Storage);
        }
        let mut names = std::collections::BTreeSet::new();
        for child in fs::read_dir(entry.path()).map_err(|_| Error::Storage)? {
            let child = child.map_err(|_| Error::Storage)?;
            if !fs::symlink_metadata(child.path())
                .map_err(|_| Error::Storage)?
                .is_file()
            {
                return Err(Error::Storage);
            }
            names.insert(child.file_name());
            if names.len() > 3 {
                return Err(Error::Storage);
            }
        }
        let expected: std::collections::BTreeSet<_> =
            ["app-release.json", "bundle.json", "payload"]
                .map(std::ffi::OsString::from)
                .into_iter()
                .collect();
        if names != expected
            || digest(&read_bounded(
                &entry.path().join("app-release.json"),
                MANIFEST_BYTES,
            )?) != name
        {
            return Err(Error::Storage);
        }
        fs::remove_dir_all(entry.path()).map_err(|_| Error::Storage)?;
    }
    Ok(())
}
fn local_candidate(
    path: &Path,
    identity: String,
    policy: &Policy,
    cancel: &CancellationToken,
) -> Result<Candidate, Error> {
    if !fs::symlink_metadata(path)
        .map_err(|_| Error::Storage)?
        .is_dir()
        || !fs::symlink_metadata(path.join("payload"))
            .map_err(|_| Error::Storage)?
            .is_file()
    {
        return Err(Error::Storage);
    }
    let candidate = verify(
        read_bounded(&path.join("app-release.json"), MANIFEST_BYTES)?,
        read_bounded(&path.join("bundle.json"), BUNDLE_BYTES)?,
        policy,
    )?;
    if candidate.identity != identity {
        return Err(Error::Storage);
    }
    let file = File::open(path.join("payload")).map_err(|_| Error::Storage)?;
    if !file.metadata().map_err(|_| Error::Storage)?.is_file() {
        return Err(Error::Storage);
    }
    let mut file = file.take(candidate.release.artifact.bytes + 1);
    let mut bytes = [0; CHUNK];
    let mut count = 0u64;
    let mut hash = Sha256::new();
    loop {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let n = file.read(&mut bytes).map_err(|_| Error::Storage)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        hash.update(&bytes[..n]);
    }
    if count != candidate.release.artifact.bytes
        || format!("{:x}", hash.finalize()) != candidate.release.artifact.sha256
    {
        return Err(Error::Manifest);
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests;
