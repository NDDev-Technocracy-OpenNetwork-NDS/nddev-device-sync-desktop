//! Presentation shape only. FRB checks mirrors against the canonical core types;
//! no domain methods or query/observation policy are exported to Dart here.
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
pub use nddev_device_sync_domain::{
    modules::{ModuleError, ModuleObservation},
    state::Reachability,
};

#[cfg_attr(feature = "flutter", frb(mirror(ModuleObservation)))]
pub struct _ModuleObservation {
    pub id: String,
    pub version: String,
    pub configured: bool,
    pub reachability: Reachability,
    pub observed_at_ms: Option<u64>,
    pub error: Option<ModuleError>,
}

#[cfg_attr(feature = "flutter", frb(mirror(ModuleError)))]
pub enum _ModuleError {
    UnsupportedPlatform,
    Unavailable,
    Busy,
    InvalidInput,
    Timeout,
    Cancelled,
    ProviderFailed,
    InvalidResponse,
    InvalidContract,
}

#[cfg_attr(feature = "flutter", frb(mirror(Reachability)))]
pub enum _Reachability {
    NotChecked,
    Reachable,
    Unavailable,
    Failed,
}
