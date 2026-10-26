//! Build metadata for the owning publisher; no store inspection or release grant.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() -> std::process::ExitCode {
    let Some(source_commit) = option_env!("NDS_SOURCE_COMMIT").filter(|value| {
        value.len() == 40
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) else {
        eprintln!("App release metadata requires an immutable compiled source commit");
        return std::process::ExitCode::FAILURE;
    };

    let metadata = serde_json::json!({
        "schema": 1,
        "source_commit": source_commit,
        "component_version": env!("CARGO_PKG_VERSION"),
        "platform": "linux",
        "architecture": "x86_64",
        "data": {
            "vault_journal": nddev_device_sync_vault::journal_schema_version(),
            "sync_wire": nddev_device_sync_accounts::sync_wire_schema_version(),
            "accounts_fence": nddev_device_sync_accounts::authorization_fence_schema_version(),
        },
    });
    println!("{metadata}");
    std::process::ExitCode::SUCCESS
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() -> std::process::ExitCode {
    eprintln!("App release metadata is supported only for Linux x86_64");
    std::process::ExitCode::FAILURE
}
