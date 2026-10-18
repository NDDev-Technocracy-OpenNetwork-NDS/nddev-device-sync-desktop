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
#[ignore = "requires an explicitly owned loopback PG/SMTP fixture and isolated native credentials"]
fn hidden_desktop_replays_without_presentation_ticks() {
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
            client.prepare_device("Owned replica".into()).await.unwrap();
            client.set_lifecycle(ClientLifecycle::Resumed);
            ready(&client, 0).await;

            // Detached has no granted process lifetime. Stage real encrypted
            // work locally; no presentation reconcile or manual sync follows.
            client.set_lifecycle(ClientLifecycle::Detached);
            tokio::time::sleep(Duration::from_millis(500)).await;
            let queued = client
                .vault_save(
                    None,
                    0,
                    None,
                    "Hidden replica".into(),
                    String::new(),
                    "SYNTHETIC_REPLICA_VALUE".into(),
                )
                .await
                .unwrap();
            assert_eq!(queued.pending, 1);
            tokio::time::sleep(Duration::from_secs(1)).await;
            ready(&client, 1).await;

            client.set_lifecycle(ClientLifecycle::Hidden);
            assert!(!client.catalog_active());
            ready(&client, 0).await;
            let synced = client.vault_snapshot().await.unwrap();
            assert!(synced.last_sync_at > 0);
            assert!(synced.sync_error.is_empty());
            assert_eq!(synced.conflicts, 0);
            assert!(client.snapshot().session_verified);
            std::fs::write(
                directory.join("proof.json"),
                serde_json::to_vec(&json!({
                    "detached_pending": queued.pending,
                    "hidden_pending": synced.pending,
                    "conflicts": synced.conflicts,
                    "interactive_admitted": client.catalog_active(),
                    "session_verified_without_ui_tick": client.snapshot().session_verified,
                    "manual_sync": false
                }))
                .unwrap(),
            )
            .unwrap();
            client.cancel();
        });
}
