//! Positive, owned Linux screen acceptance. The external fixture supplies real
//! SMTP OTPs and changes company roles through signed production routes.
use super::{identity::NdsClient, telemetry};
use nddev_device_sync_accounts::view::ClientError;
use nddev_device_sync_telemetry::view::ClientLifecycle;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

async fn wait_file(path: &Path) -> Value {
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            if let Ok(bytes) = std::fs::read(path) {
                assert!(bytes.len() <= 4096);
                break serde_json::from_slice(&bytes).unwrap();
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("owned fixture handoff timed out")
}
fn publish(root: &Path, name: &str, value: Value) {
    let temporary = root.join(format!("{name}.tmp"));
    std::fs::write(&temporary, serde_json::to_vec(&value).unwrap()).unwrap();
    std::fs::rename(temporary, root.join(name)).unwrap();
}
async fn prepare(client: &NdsClient) {
    // Enrollment starts the ordinary replica owner. Wait for its real protected
    // key provisioning before attempting encrypted module settings.
    for attempt in 0..20 {
        match client.vault_snapshot().await {
            Ok(view) if view.unlocked => break,
            Ok(_) | Err(nddev_device_sync_vault::view::VaultError::Busy) if attempt < 19 => {
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            other => {
                eprintln!(
                    "NDS_SCREEN_VAULT_NOT_READY error={}",
                    other
                        .err()
                        .map_or("key_unavailable".to_owned(), |e| e.code())
                );
                panic!("actual protected Vault did not become ready");
            }
        }
    }
    for attempt in 0..8 {
        let granted = async {
            client.reconcile().await?;
            let module = client
                .device_modules()
                .await?
                .into_iter()
                .find(|m| m.id == "remote")
                .unwrap();
            client
                .configure_module(
                    "remote".into(),
                    true,
                    vec!["read_only".into(), "local_network".into()],
                    module.revision,
                    module.pending_id,
                )
                .await
        }
        .await;
        match granted {
            Ok(()) => break,
            Err(ClientError::Busy) if attempt < 7 => {
                tokio::time::sleep(Duration::from_millis(400)).await
            }
            Err(error) => {
                eprintln!("NDS_SCREEN_CONSENT_REFUSED code={}", error.code());
                panic!("actual remote module consent failed: {}", error.code());
            }
        }
    }
    for attempt in 0..8 {
        match client.screen_prepare().await {
            Ok(()) => break,
            Err(super::screen::ScreenFailure::Busy) if attempt < 7 => {
                tokio::time::sleep(Duration::from_millis(400)).await
            }
            Err(_) => panic!("actual screen preparation failed"),
        }
    }
    for attempt in 0..8 {
        match client.device_modules().await {
            Ok(modules) => {
                let module = modules.iter().find(|m| m.id == "remote").unwrap();
                assert_eq!(module.readiness, "ready");
                assert!(module.checked_at_ms.is_some());
                return;
            }
            Err(ClientError::Busy) if attempt < 7 => {
                tokio::time::sleep(Duration::from_millis(400)).await
            }
            Err(error) => panic!(
                "actual remote module readiness read failed: {}",
                error.code()
            ),
        }
    }
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_company_screen_revalidation_closes_native_stream() {
    screen_case("role");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_source_loss_closes_native_stream() {
    screen_case("source-loss");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_cancellation_closes_native_stream() {
    screen_case("cancel");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_local_disable_closes_native_stream() {
    screen_case("local-disable");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_logout_closes_native_stream() {
    screen_case("logout");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and a real five-minute view lifetime"]
fn actual_expiry_closes_native_stream() {
    screen_case("expiry");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_role_cleanup_error_survives_device_and_identity_shutdown() {
    screen_case("role-cleanup-failure");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_device_cleanup_error_survives_concurrent_close_waiters() {
    screen_case("device-cleanup-failure");
}

#[test]
#[ignore = "requires owned PG/SMTP, separate stores and an owned paused/resumed X11 display"]
fn actual_lost_host_completion_retains_original_owner() {
    screen_case("host-completion-loss");
}

async fn logout(client: &NdsClient) {
    for attempt in 0..8 {
        match client.logout().await {
            Ok(_) => return,
            Err(ClientError::Busy) if attempt < 7 => {
                tokio::time::sleep(Duration::from_millis(400)).await
            }
            Err(_) => panic!("owned native session cleanup failed"),
        }
    }
}

fn screen_case(expected_case: &str) {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap().block_on(async {
        let config = std::env::var("NDS_SCREEN_ACCEPTANCE").expect("explicit owned fixture required");
        let config: Value = serde_json::from_slice(&std::fs::read(config).unwrap()).unwrap();
        let case = config["case"].as_str().unwrap_or("role");
        assert_eq!(case, expected_case);
        let cleanup_failure = matches!(case, "role-cleanup-failure" | "device-cleanup-failure");
        let completion_loss = case == "host-completion-loss";
        let endpoint = config["endpoint"].as_str().unwrap();
        assert!(endpoint.starts_with("http://127.0.0.1:") || endpoint.starts_with("http://localhost:"));
        let email = config["email"].as_str().unwrap();
        assert!(email.ends_with("@example.invalid"));
        let role = config["role"].as_str().unwrap();
        assert!(matches!(role,"host"|"viewer"));
        let root = Path::new(config["directory"].as_str().unwrap());
        assert!(root.is_absolute() && root.is_dir());
        assert_eq!(std::env::var("NDS_OWNED_XVFB").as_deref(), Ok("1"));
        let display_pid: u32 = std::env::var("NDS_OWNED_XVFB_PID").unwrap().parse().unwrap();
        let display = std::env::var("DISPLAY").unwrap();
        let command = std::fs::read(format!("/proc/{display_pid}/cmdline")).unwrap();
        let arguments = command.split(|byte| *byte==0).collect::<Vec<_>>();
        assert!(arguments.first().is_some_and(|name| name.ends_with(b"Xvfb")));
        assert!(arguments.contains(&display.as_bytes()));
        telemetry::init_app().unwrap();
        let client = NdsClient::new(endpoint.into()).unwrap();
        assert!(!client.refresh().await.unwrap().authenticated);
        client.request_email(email.into(),"en".into()).await.unwrap();
        publish(root,&format!("{role}-otp-requested.json"),json!({"requested":true}));
        let otp_path = root.join(format!("{role}-otp.json"));
        let mut otp = wait_file(&otp_path).await;
        let code = otp["code"].take().as_str().unwrap().to_owned();
        assert!(code.len()==8 && code.bytes().all(|b| b.is_ascii_digit()));
        std::fs::remove_file(otp_path).unwrap();
        assert!(client.verify_email(code).await.unwrap().authenticated);
        let view = client.prepare_device(format!("Owned screen {role}")).await.unwrap();
        let device = view.current_device_id;
        assert!(!device.is_empty());
        client.set_lifecycle(ClientLifecycle::Resumed);
        prepare(&client).await;
        publish(root,&format!("{role}-device.json"),json!({"device":device}));
        if role=="host" {
            let request = tokio::time::timeout(Duration::from_secs(90),async {
                loop {
                    if let Ok(Some(request)) = client.screen_incoming().await { break request; }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }).await.expect("actual viewer request did not arrive");
            let expires_at_ms = request.expires_at_ms;
            let mut accepted = None;
            for attempt in 0..8 {
                match client.screen_accept(request.id.clone()).await {
                    Ok(host) => { accepted = Some(host); break; }
                    Err(super::screen::ScreenFailure::Busy) if attempt < 7 => {
                        // Retry the same explicit action; never acquire a new
                        // request or renew its original grant/deadline.
                        eprintln!("NDS_SCREEN_ACCEPT_BUSY_RETRY attempt={}", attempt + 1);
                        tokio::time::sleep(Duration::from_millis(400)).await;
                    }
                    Err(error) => {
                        eprintln!("NDS_SCREEN_ACCEPT_REFUSED code={}", error.code());
                        panic!("actual host consent failed");
                    }
                }
            }
            let host = accepted.expect("actual host consent was not obtained");
            let injected = if cleanup_failure { Some(host.fail_after_join_for_fixture().await) } else { None };
            let purpose_busy_before = if cleanup_failure {
                matches!(client.shared_identity().remote_endpoint_key().await, Err(ClientError::Busy))
            } else { false };
            // Xvfb resets its root when its last client leaves. Paint only after
            // the production capturer owns a live connection, then observe the change.
            wait_file(&root.join("viewer-capture.json")).await;
            assert!(std::process::Command::new("xsetroot").args(["-solid","#ee2222"]).status().unwrap().success());
            publish(root,"host-painted.json",json!({"painted":true}));
            wait_file(&root.join("viewer-frame.json")).await;
            publish(root,"host-streaming.json",json!({"streaming":true}));
            wait_file(&root.join("end-trigger.json")).await;
            let at=std::time::Instant::now();
            if completion_loss {
                use super::screen::ScreenFailure;
                assert!(wait_file(&root.join("capture-paused.json")).await["paused"].as_bool().unwrap());
                let busy_before = matches!(client.shared_identity().remote_endpoint_key().await, Err(ClientError::Busy));
                // Abort only this registered outer task while it is still
                // running. Its handle is consumed by the production join.
                host.abort_running_host_for_fixture().await;
                let first = client.screen_close().await;
                let held_before = host.retains_unresolved_owner_for_fixture();
                let purpose_held = matches!(client.shared_identity().remote_endpoint_key().await, Err(ClientError::Busy));
                tokio::time::timeout(Duration::from_secs(1), host.closed()).await.expect("lost host admission stayed open");
                assert!(matches!(first, Err(ScreenFailure::Unresolved | ScreenFailure::Timeout)));
                if first == Err(ScreenFailure::Unresolved) { assert!(held_before); }
                publish(root,"host-unresolved.json",json!({"join_not_confirmed":true,"admission_closed":true,
                    "first_close_code":first.unwrap_err().code(),"purpose_still_busy":purpose_held}));
                wait_file(&root.join("source-resumed.json")).await;
                let (second, third) = tokio::join!(client.screen_close(), host.close());
                let replacement_refused = client.screen_prepare().await.is_err();
                let shutdown = client.shutdown().await;
                let refused = matches!(shutdown, Err(ClientError::Server(ref code)) if code == "screen_cleanup_unresolved");
                let held_after = host.retains_unresolved_owner_for_fixture();
                let not_acknowledged = !client.acknowledge_joined_screen_for_fixture();
                client.cancel();
                assert!(host.retains_unresolved_owner_for_fixture());
                for result in [second, third] { assert_eq!(result, Err(ScreenFailure::Unresolved)); }
                assert!(busy_before && purpose_held && held_after && replacement_refused && refused && not_acknowledged);
                publish(root,"host-proof.json",json!({"stopped":false,"positive_module_shutdown":false,
                    "completion_unresolved":true,"original_owner_retained_after_source_resume":true,
                    "first_close_code":first.unwrap_err().code(),
                    "purpose_busy_before_shutdown":true,"replacement_refused":true,"identity_shutdown_refused_positive":true,
                    "fixture_acknowledgement_refused":true,"observed_hold_ms":at.elapsed().as_millis()}));
                // The disposable process ends with an unresolved product owner.
                // No successful product drain or recovery is manufactured.
                return;
            }
            if case == "cancel" || cleanup_failure { host.cancel(); }
            if case == "logout" { logout(&client).await; }
            if case == "local-disable" {
                for attempt in 0..8 {
                    let result = async {
                        let module = client.device_modules().await?.into_iter().find(|m|m.id=="remote").unwrap();
                        client.configure_module("remote".into(),false,vec![],module.revision,module.pending_id).await
                    }.await;
                    match result {
                        Ok(())=>break,
                        Err(ClientError::Busy)if attempt<7=>tokio::time::sleep(Duration::from_millis(400)).await,
                        Err(error)=>panic!("owned local disable failed: {}",error.code()),
                    }
                }
            }
            tokio::time::timeout(Duration::from_secs(if case=="expiry" {330} else {10}),host.closed()).await.expect("host kept streaming after lifetime loss");
            if case == "expiry" {
                let ended = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
                // Native grants use seconds; NDS requests use milliseconds.
                assert!(ended >= u128::from(expires_at_ms / 1000 * 1000),"stream ended before the actual native expiry boundary");
                assert!(ended <= u128::from(expires_at_ms) + 10_000,"stream outlived the original request deadline");
            }
            if cleanup_failure {
                use super::screen::ScreenFailure;
                let first = if case == "role-cleanup-failure" {
                    host.close().await
                } else {
                    client.screen_close().await
                };
                let (second, third) = tokio::join!(client.screen_close(), client.screen_close());
                let repeated_role = host.close().await;
                let joined = host.device_joined_for_fixture();
                let purpose_released = client.shared_identity().remote_endpoint_key().await.is_ok();
                let shutdown = client.shutdown().await;
                let refused = matches!(shutdown, Err(ClientError::Server(ref code)) if code == "screen_failed");
                let acknowledged = client.acknowledge_joined_screen_for_fixture();
                // Only this fixture's positively drained terminal record is
                // acknowledged for teardown; no production retry/recovery API.
                if acknowledged { logout(&client).await; } else { client.cancel(); }
                let injected_after_real_join = injected.unwrap().load(std::sync::atomic::Ordering::Acquire);
                assert!(injected_after_real_join && joined && purpose_busy_before && purpose_released);
                for result in [first, second, third, repeated_role] {
                    assert_eq!(result, Err(ScreenFailure::Failed));
                }
                assert!(refused && acknowledged);
                publish(root,"host-proof.json",json!({"stopped":true,"observed_stop_ms":at.elapsed().as_millis(),
                    "known_cleanup_error":"screen_failed","injected_after_actual_native_join":true,
                    "all_original_close_waiters_retained_error":true,"device_joined":true,
                    "purpose_busy_before_and_released_after_join":true,"identity_shutdown_refused_positive":true,
                    "fixture_acknowledged_only_joined_terminal_for_teardown":true}));
            } else {
                host.close().await.unwrap();
                publish(root,"host-proof.json",json!({"stopped":true,"observed_stop_ms":at.elapsed().as_millis()}));
            }
        } else {
            let host = wait_file(&root.join("host-device.json")).await;
            let viewer = client.screen_open(host["device"].as_str().unwrap().into()).await.expect("actual native viewer failed");
            publish(root,"viewer-stage.json",json!({"stage":"connected"}));
            let first = viewer.frame().await.expect("initial actual frame did not arrive");
            first.check_live().unwrap();
            drop(first);
            publish(root,"viewer-capture.json",json!({"capturing":true}));
            wait_file(&root.join("host-painted.json")).await;
            let frame = tokio::time::timeout(Duration::from_secs(5),async {
                loop {
                    let frame=viewer.frame().await.expect("changed actual frame did not arrive");
                    let layout=frame.layout().unwrap();
                    let bytes=frame.read_once().await.unwrap();
                    assert!(layout.width>0 && layout.height>0 && bytes.len()==(layout.stride*layout.height) as usize);
                    let red=bytes.as_chunks::<4>().0.iter().filter(|p| p[2]>150 && p[1]<90 && p[0]<90).count();
                    let facts=json!({"width":layout.width,"height":layout.height,"red_pixels":red});
                    publish(root,"viewer-last-frame.json",facts.clone());
                    if red>bytes.len()/8 {
                        publish(root,"viewer-frame.json",facts);
                        break frame;
                    }
                }
            }).await.expect("actual owned X11 color change was not visible");
            wait_file(&root.join("end-trigger.json")).await;
            let at=std::time::Instant::now();
            tokio::time::timeout(Duration::from_secs(if case=="expiry" {330} else {10}),async {
                if case=="expiry" {
                    loop {
                        tokio::select! { biased;
                            _=viewer.closed()=>break,
                            _=tokio::time::sleep(Duration::from_millis(100))=> {
                                match viewer.frame().await {
                                    Ok(current) if current.check_live().is_ok()=>{},
                                    _=>{viewer.closed().await;break;},
                                }
                            },
                        }
                    }
                } else {viewer.closed().await;}
            }).await.expect("viewer retained frames after lifetime loss");
            assert!(frame.check_live().is_err(),"retained frame survived remote revocation");
            drop(frame);
            viewer.close().await.unwrap();
            publish(root,"viewer-proof.json",json!({"stopped":true,"retained_frame_unusable_after_stop":true,"observed_stop_ms":at.elapsed().as_millis()}));
        }
        if !(cleanup_failure && role == "host") {
            client.screen_close().await.unwrap();
            logout(&client).await;
        }
        telemetry::shutdown_telemetry().await.unwrap();
    });
}
