//! Device control (ADR 0018) through the ordinary public client against an owned
//! loopback PG/SMTP fixture and the installed native system-information tool.
//! No private hooks: admission, refusal and replacement use the product APIs.
use super::{identity::NdsClient, telemetry};
use nddev_device_sync_accounts::ClientError;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

async fn wait_file(path: &Path) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(bytes) = std::fs::read(path) {
                break bytes;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("owned controller did not deliver its input")
}

fn refused<T>(result: Result<T, ClientError>) -> String {
    match result {
        Err(ClientError::Server(code)) => code,
        Err(error) => panic!("unexpected client error: {error}"),
        Ok(_) => panic!("operation was admitted"),
    }
}

#[test]
#[ignore = "requires an owned loopback PG/SMTP fixture, isolated credentials and the installed sysinfo tool"]
fn device_control_admits_narrows_and_widens_a_real_system_tool() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let config = std::env::var("NDS_CONTROL_ACCEPTANCE").expect("owned fixture required");
            let config: Value = serde_json::from_slice(&std::fs::read(config).unwrap()).unwrap();
            let endpoint = config["endpoint"].as_str().unwrap();
            assert!(endpoint.starts_with("http://127.0.0.1:"));
            let email = config["email"].as_str().unwrap();
            assert!(email.ends_with(".invalid"));
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
            let mut value: Value = serde_json::from_slice(&wait_file(&otp).await).unwrap();
            let code = value["code"].take().as_str().unwrap().to_owned();
            std::fs::remove_file(otp).unwrap();
            assert!(client.verify_email(code).await.unwrap().authenticated);
            let view = client.prepare_device("Owned control".into()).await.unwrap();
            assert!(!view.current_device_id.is_empty());
            // Module configuration uses the encrypted Vault journal.
            tokio::time::timeout(Duration::from_secs(60), async {
                loop {
                    if let Ok(vault) = client.vault_snapshot().await
                        && vault.unlocked
                        && vault.sync_error.is_empty()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            })
            .await
            .expect("owned Vault did not unlock");

            // A new enrollment is authenticated fullauto/desktop at revision 1.
            let enrolled = client.device_control().await.unwrap();
            assert_eq!(
                (
                    enrolled.mode.as_str(),
                    enrolled.revision,
                    enrolled.source.as_str()
                ),
                ("fullauto", 1, "server")
            );
            assert_eq!(enrolled.purposes, ["desktop"]);
            let agent = client.discover_local_agent().await.unwrap();
            let unset = client
                .device_modules()
                .await
                .unwrap()
                .into_iter()
                .find(|module| module.id == "sysinfo")
                .unwrap();
            assert!(!unset.enabled && unset.permissions.is_empty());
            assert_ne!(unset.readiness, "disabled");
            // No module checklist and no prompt: the installed tool answers.
            let sample = agent.system_info(&client).await.unwrap();
            assert_eq!(sample.v, 1);

            // Narrowing to manual closes fullauto admission; the checklist applies.
            let manual = client
                .set_device_control(
                    1,
                    "manual".into(),
                    vec!["desktop".into(), "dev-desktop".into()],
                )
                .await
                .unwrap();
            assert_eq!((manual.mode.as_str(), manual.revision), ("manual", 2));
            assert_eq!(refused(agent.system_info(&client).await), "module_disabled");
            // A stale draft cannot overwrite the newer revision.
            assert_eq!(
                refused(
                    client
                        .set_device_control(1, "fullauto".into(), vec!["desktop".into()])
                        .await
                ),
                "operation_conflict"
            );
            // Explicit manual consent still works exactly as before.
            let disabled = client
                .device_modules()
                .await
                .unwrap()
                .into_iter()
                .find(|module| module.id == "sysinfo")
                .unwrap();
            client
                .configure_module(
                    "sysinfo".into(),
                    true,
                    disabled.required_permissions.clone(),
                    disabled.revision,
                    disabled.pending_id.clone(),
                )
                .await
                .unwrap();
            assert_eq!(agent.system_info(&client).await.unwrap().v, 1);
            let enabled = client
                .device_modules()
                .await
                .unwrap()
                .into_iter()
                .find(|module| module.id == "sysinfo")
                .unwrap();
            client
                .configure_module(
                    "sysinfo".into(),
                    false,
                    Vec::new(),
                    enabled.revision,
                    enabled.pending_id.clone(),
                )
                .await
                .unwrap();
            assert_eq!(refused(agent.system_info(&client).await), "module_disabled");

            // Widening within the recent sign-in window restores fullauto; the
            // stored explicit disable remains for a later manual choice.
            let widened = client
                .set_device_control(2, "fullauto".into(), vec!["dev-desktop".into()])
                .await
                .unwrap();
            assert_eq!((widened.mode.as_str(), widened.revision), ("fullauto", 3));
            assert_eq!(agent.system_info(&client).await.unwrap().v, 1);
            assert!(
                !client
                    .device_modules()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|module| module.id == "sysinfo")
                    .unwrap()
                    .enabled
            );

            // The controller ages the session proof; widening then needs a new sign-in.
            client
                .set_device_control(3, "manual".into(), vec!["dev-desktop".into()])
                .await
                .unwrap();
            std::fs::write(directory.join("age-requested"), b"requested").unwrap();
            wait_file(&directory.join("aged")).await;
            assert_eq!(
                refused(
                    client
                        .set_device_control(4, "fullauto".into(), vec!["dev-desktop".into()])
                        .await
                ),
                "recent_authentication_required"
            );
            let retained = client.device_control().await.unwrap();
            assert_eq!((retained.mode.as_str(), retained.revision), ("manual", 4));
            assert_eq!(refused(agent.system_info(&client).await), "module_disabled");

            agent.shutdown().await.unwrap();
            let diagnostics = telemetry::client_diagnostics();
            std::fs::write(
                directory.join("proof.json"),
                serde_json::to_vec(&json!({
                    "device": view.current_device_id,
                    "enrolled": [enrolled.mode, enrolled.revision],
                    "fullauto_without_checklist": true,
                    "manual_refusal": "module_disabled",
                    "stale_cas": "operation_conflict",
                    "manual_explicit_consent": true,
                    "widened_revision": widened.revision,
                    "aged_widening": "recent_authentication_required",
                    "final": [retained.mode, retained.revision],
                    "rejected_fields_or_events": diagnostics.rejected_fields_or_events,
                    "dropped_events": diagnostics.dropped_events,
                }))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(diagnostics.rejected_fields_or_events, Some(0));
            client.cancel();
            telemetry::shutdown_telemetry().await.unwrap();
        });
}
