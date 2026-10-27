//! Presentation shape only. FRB checks mirrors against the canonical owning types;
//! no domain methods or query/observation policy are exported to Dart here.
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
pub use nddev_device_sync_agent::view::cache::Status as CacheStatus;
pub use nddev_device_sync_agent::view::{
    BrowserDocument,
    browser::{
        Outcome as BrowserOutcome, ScriptFailure as BrowserScriptFailure,
        Session as BrowserSession, Status as BrowserStatus, Tab as BrowserTab,
    },
};
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

// These names alias the owning browser types. Only their checked presentation
// shapes cross the bridge; owned handles and protocol frames remain in Rust.
#[cfg_attr(feature = "flutter", frb(mirror(BrowserStatus)))]
pub struct _BrowserStatus {
    pub enabled: bool,
    pub browser: String,
    pub extension: String,
    pub sessions: usize,
}

#[cfg_attr(feature = "flutter", frb(mirror(BrowserSession)))]
pub struct _BrowserSession {
    pub project: String,
    pub tabs: usize,
    pub executor: bool,
}

#[cfg_attr(feature = "flutter", frb(mirror(BrowserTab)))]
pub struct _BrowserTab {
    pub target: String,
    pub document: Option<String>,
    pub url: String,
    pub title: String,
}

#[cfg_attr(feature = "flutter", frb(mirror(BrowserOutcome)))]
pub struct _BrowserOutcome {
    pub failure: Option<BrowserScriptFailure>,
    pub output: String,
    pub truncated: bool,
    pub tabs: Vec<BrowserTab>,
}

#[cfg_attr(feature = "flutter", frb(mirror(BrowserScriptFailure)))]
pub enum _BrowserScriptFailure {
    ScriptError,
    TargetClosed,
    BrowserDisconnected,
}

#[cfg_attr(feature = "flutter", frb(mirror(BrowserDocument)))]
pub struct _BrowserDocument {
    pub target: String,
    pub document: String,
}

// Binding contains an owning static string. Keep that owner type opaque and
// expose its read-only presentation through the canonical field accessors.
#[cfg_attr(feature = "flutter", frb(mirror(CacheStatus)))]
pub struct _CacheStatus {
    pub port: u16,
    pub ready: bool,
    pub bindings: Vec<nddev_device_sync_agent::view::cache::Binding>,
}
