//! Ordinary public-API replication against owned PG/SMTP and native credentials.
//! No private startup, lifetime-race or storage-fault hooks are used.
use super::{identity::NdsClient, telemetry};
use nddev_device_sync_telemetry::view::ClientLifecycle;
use nddev_device_sync_vault::view::VaultError;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

async fn ready(client: &NdsClient, pending: u32) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match client.vault_snapshot().await {
                Ok(view) if view.unlocked && view.pending == pending => return,
                Ok(_) | Err(VaultError::Busy) => {}
                Err(error) => panic!("ordinary replica read failed: {}", error.code()),
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("owned replica did not reach its expected durable state");
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[ignore = "requires an explicitly owned loopback PG/SMTP fixture and isolated native credentials"]
fn detached_desktop_replays_without_presentation_owner() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let config = std::env::var("NDS_REPLICA_ACCEPTANCE").expect("owned fixture required");
            let config: Value = serde_json::from_slice(&std::fs::read(config).unwrap()).unwrap();
            let endpoint = config["endpoint"].as_str().unwrap();
            assert!(endpoint.starts_with("http://127.0.0.1:"));
            let email = config["email"].as_str().unwrap();
            assert!(email.ends_with("@example.invalid"));
            let directory = Path::new(config["directory"].as_str().unwrap());
            assert!(directory.is_absolute() && directory.is_dir());
            telemetry::init_app().unwrap();
            let client = NdsClient::new(endpoint.into()).unwrap();
            assert!(!client.refresh().await.unwrap().authenticated);
            client
                .request_email(email.into(), "en".into())
                .await
                .unwrap();
            std::fs::write(directory.join("otp-requested"), b"requested").unwrap();
            let otp = directory.join("otp.json");
            let mut value: Value = tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    if let Ok(bytes) = std::fs::read(&otp) {
                        assert!(bytes.len() <= 128);
                        break serde_json::from_slice(&bytes).unwrap();
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await
            .expect("owned SMTP OTP was not delivered");
            let code = value["code"].take().as_str().unwrap().to_owned();
            std::fs::remove_file(otp).unwrap();
            assert!(code.len() == 8 && code.bytes().all(|b| b.is_ascii_digit()));
            assert!(client.verify_email(code).await.unwrap().authenticated);
            let receipt = Path::new(config["receipt"].as_str().expect("owned receipt required"));
            admit_fixture(&client, receipt, "Owned replica").await;
            client.set_lifecycle(ClientLifecycle::Resumed);
            std::fs::write(directory.join("offline-requested"), b"ready").unwrap();
            wait_marker(directory, "offline-ready").await;
            client.set_lifecycle(ClientLifecycle::Detached);
            let queued = client
                .vault_save(
                    None,
                    0,
                    None,
                    "Detached replica".into(),
                    String::new(),
                    "SYNTHETIC_REPLICA_VALUE".into(),
                )
                .await
                .unwrap();
            assert_eq!(queued.pending, 1);
            assert!(!client.catalog_active());
            let retained = nddev_device_sync_vault::replication::native::NativeReplica::attach(
                client.shared_identity(),
                nddev_device_sync_accounts::private_data_directory().unwrap(),
                None,
            )
            .await
            .unwrap();
            let lifetime = std::sync::Arc::downgrade(&retained);
            drop(retained);
            client.cancel();
            drop(client);
            assert!(lifetime.upgrade().is_some_and(|owner| owner.is_running()));
            std::fs::write(directory.join("facade-detached"), b"ready").unwrap();
            // The controller restores the actual server and observes durable
            // pending0 before any replacement client is constructed.
            wait_marker(directory, "replayed").await;
            let reopened = NdsClient::new(endpoint.into()).unwrap();
            assert!(reopened.refresh().await.unwrap().authenticated);
            reopened
                .prepare_device("Owned replica".into())
                .await
                .unwrap();
            let same = nddev_device_sync_vault::replication::native::NativeReplica::attach(
                reopened.shared_identity(),
                nddev_device_sync_accounts::private_data_directory().unwrap(),
                None,
            )
            .await
            .unwrap();
            assert!(lifetime.ptr_eq(&std::sync::Arc::downgrade(&same)));
            ready(&reopened, 0).await;
            let synced = reopened.vault_snapshot().await.unwrap();
            assert!(
                synced.last_sync_at > 0 && synced.sync_error.is_empty() && synced.conflicts == 0
            );
            let records = reopened.vault_records(String::new()).await.unwrap();
            assert!(
                records
                    .items
                    .iter()
                    .any(|record| record.title == "Detached replica"
                        && record.pending_id.is_none()
                        && record.observed_seq > 0)
            );
            std::fs::write(
                directory.join("proof.json"),
                serde_json::to_vec(&json!({
                    "detached_pending": queued.pending,
                    "replayed_pending": synced.pending,
                    "owner_reused": true,
                    "replacement_created_after_replay": true,
                    "manual_sync": false,
                    "interactive_admitted_while_detached": false
                }))
                .unwrap(),
            )
            .unwrap();
            reopened.cancel();
            telemetry::shutdown_telemetry().await.unwrap();
        });
}

async fn wait_marker(directory: &Path, name: &str) {
    tokio::time::timeout(Duration::from_secs(90), async {
        while !directory.join(name).is_file() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("owned controller did not reach the expected state");
}

async fn admit_fixture(client: &NdsClient, receipt: &Path, name: &str) {
    let fixture: Value = serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
    assert_eq!(
        fixture["base_url"].as_str().unwrap(),
        client.shared_identity().origin().trim_end_matches('/')
    );
    client
        .shared_identity()
        .prepare_device(name.into())
        .await
        .unwrap();
    assert_eq!(
        client
            .shared_identity()
            .sync_request(nddev_device_sync_accounts::SyncRequest::CapabilitiesV3)
            .await
            .unwrap()
            .status,
        200
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(bytes) = std::fs::read(receipt)
                && let Ok(state) = serde_json::from_slice::<Value>(&bytes)
                && state["new_operation_version"] == 3
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    client.prepare_device(name.into()).await.unwrap();
    // Both actual startup observations must be committed before offline staging.
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let Ok(page) = client.sync_states(String::new()).await {
                let has = |kind| {
                    page.items
                        .iter()
                        .any(|state| state.kind == kind && state.pending_id.is_none())
                };
                if has("device_state") && has("account_state") {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    ready(client, 0).await;
}

// The native window acceptance runs these helpers in separate real processes.
// They use ordinary client APIs and an isolated credential service; the actual
// Flutter application must deliver the staged operation after its window hides.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires the owned native-window PG/SMTP and credential fixture"]
fn seed_native_window_profile() {
    window_case(true);
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires the owned native-window fixture with its server offline"]
fn stage_native_window_offline_work() {
    window_case(false);
}

#[cfg(target_os = "linux")]
fn window_case(seed: bool) {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let path = std::env::var("NDS_NATIVE_WINDOW_ACCEPTANCE").unwrap();
            let bytes = std::fs::read(path).unwrap();
            assert!(bytes.len() <= 8192);
            let config: Value = serde_json::from_slice(&bytes).unwrap();
            let root = Path::new(config["directory"].as_str().unwrap());
            assert!(root.is_absolute() && root.is_dir());
            let receipt = config["receipt"].as_str().unwrap();
            let fixture: Value = serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
            let endpoint = fixture["base_url"].as_str().unwrap();
            assert!(endpoint.starts_with("http://127.0.0.1:"));
            let email = fixture["owner_email"].as_str().unwrap();
            assert!(email.ends_with("@example.invalid"));
            telemetry::init_app().unwrap();
            let client = NdsClient::new(endpoint.into()).unwrap();
            client.set_lifecycle(ClientLifecycle::Detached);
            let view = client.refresh().await.unwrap();
            if seed {
                assert!(!view.authenticated);
                client
                    .request_email(email.into(), "en".into())
                    .await
                    .unwrap();
                std::fs::write(root.join("otp-requested"), b"requested").unwrap();
                let otp = root.join("otp.json");
                let code: Value = tokio::time::timeout(Duration::from_secs(20), async {
                    loop {
                        if let Ok(bytes) = std::fs::read(&otp) {
                            assert!(bytes.len() <= 128);
                            break serde_json::from_slice(&bytes).unwrap();
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
                .await
                .unwrap();
                std::fs::remove_file(otp).unwrap();
                client
                    .verify_email(code["code"].as_str().unwrap().into())
                    .await
                    .unwrap();
                admit_fixture(&client, Path::new(receipt), "Owned window acceptance").await;
                let view = client.vault_snapshot().await.unwrap();
                assert!(view.unlocked && view.pending == 0 && !view.catching_up);
                assert!(view.last_sync_at > 0 && view.sync_error.is_empty());
                std::fs::write(root.join("seeded"), b"ready").unwrap();
            } else {
                assert!(view.authenticated && view.offline_authorized && !view.session_verified);
                let view = client
                    .vault_save(
                        None,
                        0,
                        None,
                        "Owned closed-window replay".into(),
                        String::new(),
                        "SYNTHETIC_WINDOW_SECRET".into(),
                    )
                    .await
                    .unwrap();
                assert!(view.unlocked && view.pending == 1);
                std::fs::write(
                    root.join("staged.json"),
                    serde_json::to_vec(&json!({"pending":view.pending,"offline_authorized":true}))
                        .unwrap(),
                )
                .unwrap();
            }
            client.cancel();
        });
}
