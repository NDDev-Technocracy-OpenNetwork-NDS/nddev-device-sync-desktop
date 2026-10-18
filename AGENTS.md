# NDS desktop working contract

Flutter owns presentation only. Shared nds_ui and the thin generated Rust bridge
live in the desktop repository; mobile and web consume the same implementation.
NDS credentials stay in the Rust accounts module and its platform adapters. Never export tokens,
exchange tokens or private keys through FFI or log bodies. Protocol owns wire
schemas; generate mechanical projections and keep core independent of Flutter.

Read standarts.lock and module.yaml. Use the Rust compiler in
packages/nds_native/rust/rust-toolchain.toml and Flutter/Dart in toolchains.lock.json. Keep en/ru ARB messages, NDDev OpenNetwork branding and https://nddev.ai.
Public code is AGPL-3.0-only and contains no real owner addresses, infrastructure
configuration or runtime evidence. Preserve native stores and unrelated work.
Run formatting, analysis, generated-source checks and actual platform acceptance;
never represent a mock or a host-only test as another platform's acceptance.
Working branch -> dev -> main; no implicit release, deploy or backup/recovery.
