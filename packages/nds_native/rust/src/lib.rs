pub mod api;
#[cfg(all(feature = "flutter", not(frb_expand)))]
mod frb_generated;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_arch = "wasm32")]
mod browser;
mod provenance;

#[cfg(all(target_arch = "wasm32", feature = "browser-bindings"))]
mod browser_api;
