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
    for attempt in 0..8 {
        match client.screen_prepare().await {
            Ok(()) => return,
            Err(super::screen::ScreenFailure::Busy) if attempt < 7 => {
                tokio::time::sleep(Duration::from_millis(400)).await
            }
            Err(_) => panic!("actual screen preparation failed"),
        }
    }
}

#[test]
#[ignore = "requires owned PG/SMTP, separate native stores and an owned X11 display"]
fn actual_company_screen_revalidation_closes_native_stream() {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap().block_on(async {
        let config = std::env::var("NDS_SCREEN_ACCEPTANCE").expect("explicit owned fixture required");
        let config: Value = serde_json::from_slice(&std::fs::read(config).unwrap()).unwrap();
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
            let host = client.screen_accept(request.id).await.expect("actual host consent failed");
            // Xvfb resets its root when its last client leaves. Paint only after
            // the production capturer owns a live connection, then observe the change.
            wait_file(&root.join("viewer-capture.json")).await;
            assert!(std::process::Command::new("xsetroot").args(["-solid","#ee2222"]).status().unwrap().success());
            publish(root,"host-painted.json",json!({"painted":true}));
            wait_file(&root.join("viewer-frame.json")).await;
            publish(root,"host-streaming.json",json!({"streaming":true}));
            wait_file(&root.join("demoted.json")).await;
            let at=std::time::Instant::now();
            tokio::time::timeout(Duration::from_secs(10),host.closed()).await.expect("host kept streaming after demotion");
            host.close().await.unwrap();
            publish(root,"host-proof.json",json!({"stopped":true,"observed_stop_ms":at.elapsed().as_millis()}));
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
                    let bytes=frame.read_once().unwrap();
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
            wait_file(&root.join("demoted.json")).await;
            let at=std::time::Instant::now();
            tokio::time::timeout(Duration::from_secs(10),viewer.closed()).await.expect("viewer retained permission after demotion");
            assert!(frame.check_live().is_err(),"retained frame survived remote revocation");
            drop(frame);
            viewer.close().await.unwrap();
            publish(root,"viewer-proof.json",json!({"stopped":true,"retained_frame_unusable_after_stop":true,"observed_stop_ms":at.elapsed().as_millis()}));
        }
        client.screen_close().await.unwrap();
        for attempt in 0..8 {
            match client.logout().await {
                Ok(_) => break,
                Err(ClientError::Busy) if attempt<7 => tokio::time::sleep(Duration::from_millis(400)).await,
                Err(_) => panic!("owned native session cleanup failed"),
            }
        }
        telemetry::shutdown_telemetry().await;
    });
}
