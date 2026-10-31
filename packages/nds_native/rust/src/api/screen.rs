//! Private caller proposal: one existing NdsClient slot, no Dart authority data.
use super::identity::NdsClient;
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
#[cfg(target_os = "linux")]
use tracing::{Instrument, instrument::WithSubscriber};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenFailure {
    Unsupported,
    Denied,
    Expired,
    Closed,
    Busy,
    InvalidInput,
    InvalidResponse,
    Limit,
    Timeout,
    Unavailable,
    Failed,
    Unresolved,
}
impl std::fmt::Display for ScreenFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for ScreenFailure {}
impl ScreenFailure {
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported => "screen_unsupported",
            Self::Denied => "screen_denied",
            Self::Expired => "screen_expired",
            Self::Closed => "screen_closed",
            Self::Busy => "screen_busy",
            Self::InvalidInput => "screen_invalid_input",
            Self::InvalidResponse => "screen_invalid_response",
            Self::Limit => "screen_limit",
            Self::Timeout => "screen_timeout",
            Self::Unavailable => "screen_unavailable",
            Self::Failed => "screen_failed",
            Self::Unresolved => "screen_cleanup_unresolved",
        }
    }
}

fn classify_close(error: &ScreenFailure) -> (String, &'static str, bool) {
    let code = error.code().into();
    match error {
        ScreenFailure::Expired | ScreenFailure::Closed => (code, "stopped", false),
        ScreenFailure::Timeout | ScreenFailure::Unavailable | ScreenFailure::Unresolved => {
            (code, "pending", true)
        }
        _ => (code, "error", matches!(error, ScreenFailure::Failed)),
    }
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn screen_supported() -> bool {
    cfg!(target_os = "linux")
}

pub struct ScreenLayout {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}
#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct ScreenFrame {
    #[cfg(target_os = "linux")]
    frame: Mutex<nddev_device_sync_rds::owned::runtime::FrameTicket>,
    #[cfg(target_os = "linux")]
    state: Arc<State>,
    #[cfg(target_os = "linux")]
    life: Arc<RoleLife>,
}
impl ScreenFrame {
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn layout(&self) -> Result<ScreenLayout, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            self.state.publish_frame(&self.life, || {
                let (width, height, stride) = self
                    .frame
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .dimensions()
                    .map_err(native::failure)?;
                Ok(ScreenLayout {
                    width,
                    height,
                    stride,
                })
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn check_live(&self) -> Result<(), ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            self.state.publish_frame(&self.life, || {
                self.frame
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .check_live()
                    .map_err(native::failure)
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    /// Revalidate the original protected guard after an external renderer wait.
    /// check_live remains the final synchronous publication check.
    pub async fn validate_context(&self) -> Result<(), ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            if let Err(error) = self.state.module.validate().await {
                self.life.stop.cancel();
                return Err(native::account(error));
            }
            self.check_live()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    pub async fn read_once(&self) -> Result<Vec<u8>, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            self.validate_context().await?;
            self.state.publish_frame(&self.life, || {
                self.frame
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .read_once()
                    .map_err(native::failure)
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
}

struct State {
    stop: CancellationToken,
    completion: tokio::sync::watch::Sender<Option<Result<(), ScreenFailure>>>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    joined: std::sync::atomic::AtomicBool,
    #[cfg(target_os = "linux")]
    drained: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(target_os = "linux")]
    hold: Mutex<Option<Arc<State>>>,
    role: Mutex<Option<Arc<RoleLife>>>,
    #[cfg(target_os = "linux")]
    device: tokio::sync::Mutex<Option<native::DeviceScreen>>,
    #[cfg(target_os = "linux")]
    fence: Mutex<Option<Arc<nddev_device_sync_accounts::LocalUseFence>>>,
    #[cfg(target_os = "linux")]
    module: Arc<super::module_control::ModuleUseGuard>,
    #[cfg(target_os = "linux")]
    publication: Arc<Mutex<()>>,
}
// Captured before spawn: even an unpolled/aborted/panicking future publishes a
// truthful terminal outcome. This is task completion, never authorization.
#[cfg(any(target_os = "linux", test))]
struct DeviceCompletion {
    stop: CancellationToken,
    result: tokio::sync::watch::Sender<Option<Result<(), ScreenFailure>>>,
    drained: Arc<std::sync::atomic::AtomicBool>,
    retain: Option<Box<dyn FnOnce() + Send>>,
    finished: bool,
}
#[cfg(any(target_os = "linux", test))]
impl DeviceCompletion {
    async fn run(self, work: impl std::future::Future<Output = Result<(), ScreenFailure>>) {
        self.finish(work.await);
    }
    fn finish(mut self, result: Result<(), ScreenFailure>) {
        self.stop.cancel();
        if self.drained.load(std::sync::atomic::Ordering::Acquire) {
            self.retain.take();
            self.result.send_replace(Some(result));
        } else {
            self.unresolved();
        }
        self.finished = true;
    }
    fn unresolved(&mut self) {
        self.stop.cancel();
        if let Some(retain) = self.retain.take() {
            retain();
        }
        self.result
            .send_replace(Some(Err(ScreenFailure::Unresolved)));
    }
}
#[cfg(any(target_os = "linux", test))]
impl Drop for DeviceCompletion {
    fn drop(&mut self) {
        if !self.finished {
            self.unresolved();
        }
    }
}
struct RoleLife {
    stop: CancellationToken,
    #[cfg(target_os = "linux")]
    validation: Mutex<Option<nddev_device_sync_domain::screen::ViewProjection>>,
    #[cfg(target_os = "linux")]
    done: CancellationToken,
    #[cfg(target_os = "linux")]
    terminal: Mutex<Option<Result<(), ScreenFailure>>>,
    #[cfg(target_os = "linux")]
    context: super::organizations::OrganizationObservation,
}
#[cfg(target_os = "linux")]
impl RoleLife {
    fn finish(&self, result: Result<(), ScreenFailure>) {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert(result);
        self.done.cancel();
    }
}
impl State {
    #[cfg(target_os = "linux")]
    fn publish_frame<T>(
        &self,
        role: &Arc<RoleLife>,
        work: impl FnOnce() -> Result<T, ScreenFailure>,
    ) -> Result<T, ScreenFailure> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.module
            .publish::<_, nddev_device_sync_accounts::ClientError>(|| {
                role.context.check()?;
                if self.stop.is_cancelled()
                    || role.stop.is_cancelled()
                    || self
                        .role
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_ref()
                        .is_none_or(|current| !Arc::ptr_eq(current, role))
                {
                    return Err(nddev_device_sync_accounts::ClientError::Cancelled);
                }
                Ok(work())
            })
            .map_err(native::account)?
    }
    #[cfg(target_os = "linux")]
    fn retain_unresolved(self: &Arc<Self>) {
        self.stop.cancel();
        self.cancel_role();
        *self.hold.lock().unwrap_or_else(|e| e.into_inner()) = Some(self.clone());
    }
    #[cfg(target_os = "linux")]
    fn is_drained(&self) -> bool {
        self.joined.load(std::sync::atomic::Ordering::Acquire)
            && self.drained.load(std::sync::atomic::Ordering::Acquire)
    }
    fn cancel_role(&self) {
        if let Some(role) = self.role.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            role.stop.cancel();
        }
    }
    async fn join(&self) -> Result<(), ScreenFailure> {
        self.stop.cancel();
        super::identity::observe_cleanup(
            "rds",
            "rds.device.join",
            async {
                {
                    // Borrow the registered handle: cancellation of this waiter
                    // cannot detach work or cause a completed handle to be polled twice.
                    let mut task = self.task.lock().await;
                    if let Some(work) = task.as_mut() {
                        let _ = work.await;
                        task.take();
                        self.joined
                            .store(true, std::sync::atomic::Ordering::Release);
                    }
                }
                let mut completion = self.completion.subscribe();
                loop {
                    let result = *completion.borrow_and_update();
                    if let Some(result) = result {
                        return result;
                    }
                    completion
                        .changed()
                        .await
                        .map_err(|_| ScreenFailure::Unavailable)?;
                }
            },
            classify_close,
        )
        .await
    }
    #[cfg(target_os = "linux")]
    async fn close_role(&self, role: &Arc<RoleLife>) -> Result<(), ScreenFailure> {
        role.stop.cancel();
        if let Some(result) = *role.terminal.lock().unwrap_or_else(|e| e.into_inner()) {
            return result;
        }
        let mut slot = self.device.lock().await;
        if let Some(device) = slot.as_mut() {
            return device.close_child_if(role).await;
        }
        (*self.completion.borrow()).unwrap_or(Err(ScreenFailure::Busy))
    }
}
pub(crate) struct ScreenSlot(Arc<State>);
impl Drop for ScreenSlot {
    fn drop(&mut self) {
        self.0.stop.cancel();
    }
}

#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct ScreenView {
    #[cfg(target_os = "linux")]
    state: Arc<State>,
    life: Arc<RoleLife>,
    #[cfg(target_os = "linux")]
    viewer: Arc<tokio::sync::Mutex<Option<nddev_device_sync_rds::owned::runtime::NativeViewer>>>,
}
impl ScreenView {
    pub async fn frame(&self) -> Result<ScreenFrame, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            if self.state.stop.is_cancelled() || self.life.stop.is_cancelled() {
                return Err(ScreenFailure::Closed);
            }
            let mut viewer = self.viewer.lock().await;
            let ticket = viewer
                .as_mut()
                .ok_or(ScreenFailure::Closed)?
                .pull_frame()
                .await
                .map_err(native::failure)?;
            let frame = ScreenFrame {
                frame: Mutex::new(ticket),
                state: self.state.clone(),
                life: self.life.clone(),
            };
            frame.check_live()?;
            Ok(frame)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn cancel(&self) {
        self.life.stop.cancel();
    }
    pub async fn closed(&self) {
        self.life.stop.cancelled().await;
    }
    pub async fn close(&self) -> Result<(), ScreenFailure> {
        self.cancel();
        #[cfg(target_os = "linux")]
        {
            self.state.close_role(&self.life).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
}
#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct ScreenHost {
    #[cfg(target_os = "linux")]
    state: Arc<State>,
    life: Arc<RoleLife>,
}
impl ScreenHost {
    // Fixture-only failure after the real native host has actually joined.
    // No production wire or recovery/authorization API is added.
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) async fn fail_after_join_for_fixture(&self) -> Arc<std::sync::atomic::AtomicBool> {
        let marker = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.state
            .device
            .lock()
            .await
            .as_mut()
            .expect("owned fixture device")
            .fail_after_join_for_fixture(&self.life, marker.clone());
        marker
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn device_joined_for_fixture(&self) -> bool {
        self.state.is_drained()
            && self
                .state
                .hold
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none()
            && self
                .state
                .device
                .try_lock()
                .is_ok_and(|device| device.is_none())
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn cancel(&self) {
        self.life.stop.cancel();
    }
    pub async fn closed(&self) {
        self.life.stop.cancelled().await;
    }
    pub async fn close(&self) -> Result<(), ScreenFailure> {
        self.cancel();
        #[cfg(target_os = "linux")]
        {
            self.state.close_role(&self.life).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
}
// Public request facts only; selecting this row does not grant source access.
pub struct ScreenRequest {
    pub id: String,
    pub viewer_device: String,
    pub display: u32,
    pub max_fps: u32,
    pub output_height: u32,
    pub expires_at_ms: u64,
}
pub struct ScreenTargetView {
    pub device_id: String,
    pub display_name: String,
    pub platform: String,
    pub registered: bool,
    pub is_current: bool,
}

impl NdsClient {
    // Dispose only a positively joined fixture terminal record so that the
    // test can join the remaining NdsClient resources after asserting refusal.
    // An unresolved/running owner can never pass this predicate.
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn acknowledge_joined_screen_for_fixture(&self) -> bool {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let mut slot = self.screen.lock().unwrap_or_else(|e| e.into_inner());
        let known = slot.as_ref().is_some_and(|slot| {
            slot.0.is_drained()
                && matches!(
                    *slot.0.completion.borrow(),
                    Some(Err(ScreenFailure::Failed))
                )
                && slot
                    .0
                    .device
                    .try_lock()
                    .is_ok_and(|device| device.is_none())
                && slot
                    .0
                    .hold
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_none()
        });
        if known {
            slot.take();
        }
        known
    }
    /// Metadata read only. Opening a view still rechecks current server authority
    /// and the target endpoint; registration is never capture consent or presence.
    pub async fn screen_targets(
        &self,
        user: String,
    ) -> Result<Vec<ScreenTargetView>, nddev_device_sync_accounts::ClientError> {
        use nddev_device_sync_accounts::ClientError;
        use nddev_device_sync_domain::{UserId, devices::DevicePlatform};
        let user = UserId::new(user).map_err(|_| ClientError::InvalidInput)?;
        let identity = self.shared_identity();
        let _module = self
            .module_access("rds", super::module_control::Adapter::Shared)
            .await?;
        let lifetime = identity.local_use_lifetime();
        let fence = identity.local_use_fence_from(&lifetime).await?;
        let targets = identity.screen_targets(&fence, &user).await?;
        identity.validate_local_use(&fence).await?;
        let device = identity.view().current_device_id;
        Ok(targets
            .into_iter()
            .map(|target| ScreenTargetView {
                is_current: target.device.as_str() == device,
                device_id: target.device.into(),
                display_name: target.name.as_str().into(),
                registered: target.registered,
                platform: match target.platform {
                    DevicePlatform::Android => "android",
                    DevicePlatform::Ios => "ios",
                    DevicePlatform::Linux => "linux",
                    DevicePlatform::Macos => "macos",
                    DevicePlatform::Web => "web",
                    DevicePlatform::Windows => "windows",
                }
                .into(),
            })
            .collect())
    }

    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) fn cancel_screen(&self) {
        if let Some(slot) = self
            .screen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            slot.0.stop.cancel();
        }
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn screen_cancel(&self) {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = self
            .screen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            if slot
                .0
                .role
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some()
            {
                slot.0.cancel_role();
            } else {
                slot.0.stop.cancel();
            }
        }
    }
    pub async fn screen_stop(&self) -> Result<(), ScreenFailure> {
        self.screen_cancel();
        #[cfg(target_os = "linux")]
        {
            let state = self
                .screen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map(|slot| slot.0.clone());
            if let Some(state) = state {
                let mut slot = state.device.lock().await;
                if let Some(device) = slot.as_mut() {
                    return device.close_child().await;
                }
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    pub async fn screen_close(&self) -> Result<(), ScreenFailure> {
        let state = self
            .screen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|slot| slot.0.clone());
        if let Some(state) = state {
            let result = super::identity::observe_cleanup(
                "rds",
                "rds.device.close_request",
                async {
                    tokio::time::timeout(std::time::Duration::from_secs(3), state.join())
                        .await
                        .map_err(|_| ScreenFailure::Timeout)?
                },
                classify_close,
            )
            .await;
            // Keep one exact terminal record until a later explicit admission.
            // Repeated closers must not reinterpret a known failure as success.
            return result;
        }
        Ok(())
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) async fn reconcile_screen(&self) {
        #[cfg(target_os = "linux")]
        {
            let state = self
                .screen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map(|slot| slot.0.clone());
            if let Some(state) = state {
                if state.stop.is_cancelled() {
                    let finished = state
                        .task
                        .try_lock()
                        .is_ok_and(|task| task.as_ref().is_none_or(|task| task.is_finished()));
                    if finished {
                        let _ = state.join().await;
                    }
                    return;
                }
                let fence = state
                    .fence
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if let Some(fence) = fence {
                    match self.shared_identity().validate_local_use(&fence).await {
                        Ok(()) | Err(nddev_device_sync_accounts::ClientError::Busy) => {}
                        Err(_) => state.stop.cancel(),
                    }
                }
            }
        }
    }
    // Registration is device-scoped. It starts no capture and acquires no per-role key.
    pub async fn screen_prepare(&self) -> Result<(), ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            self.screen_device(false, None).await.map(|_| ())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    pub async fn screen_incoming(&self) -> Result<Option<ScreenRequest>, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            let (state, _) = self.screen_device(false, None).await?;
            let slot = state.device.lock().await;
            let device = slot.as_ref().ok_or(ScreenFailure::Closed)?;
            let request = device
                .identity
                .screen_incoming(&device.fence)
                .await
                .map_err(native::account)?;
            Ok(request.map(|view| ScreenRequest {
                id: view.id().into(),
                viewer_device: view.permit().viewer.as_str().into(),
                display: view.permit().lease.display(),
                max_fps: view.permit().lease.max_fps(),
                output_height: view.permit().lease.output_height(),
                expires_at_ms: view.permit().lease.expires_at_ms(),
            }))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScreenFailure::Unsupported)
        }
    }
    // Explicit invocation is opt-in for this exact freshly re-read request.
    pub async fn screen_accept(&self, id: String) -> Result<ScreenHost, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            let (state, life) = self.screen_device(true, None).await?;
            let life = life.ok_or(ScreenFailure::Closed)?;
            let lifecycle = *self
                .screen_lifecycle
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let mut slot = state.device.lock().await;
            if life.stop.is_cancelled() {
                return Err(ScreenFailure::Closed);
            }
            let device = slot.as_mut().ok_or(ScreenFailure::Closed)?;
            if device
                .pending_host
                .as_ref()
                .is_some_and(|pending| pending.request.id() == id)
            {
                // Same uncertain Ready retry keeps its exact grant/request.
                device
                    .close_active()
                    .await
                    .inspect_err(|_| life.stop.cancel())?;
            } else {
                device
                    .close_child()
                    .await
                    .inspect_err(|_| life.stop.cancel())?;
            }
            let life = device.accept(&state, life, &id, lifecycle).await?;
            let error = {
                match self.screen_publication(&state, Some(&life)) {
                    Ok(_publication) => {
                        return Ok(ScreenHost {
                            state: state.clone(),
                            life,
                        });
                    }
                    Err(error) => error,
                }
            };
            device.close_child_if(&life).await?;
            Err(error)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = id;
            Err(ScreenFailure::Unsupported)
        }
    }
    pub async fn screen_open(&self, target: String) -> Result<ScreenView, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            self.screen_open_scoped(target, None).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = target;
            Err(ScreenFailure::Unsupported)
        }
    }
    /// The original organization handle supplies only its captured context;
    /// Accounts and the target still own remote screen authorization.
    pub async fn screen_open_for_organization(
        &self,
        organization: &super::organizations::LocalOrganization,
        target: String,
    ) -> Result<ScreenView, ScreenFailure> {
        #[cfg(target_os = "linux")]
        {
            let original = organization.observe_for(self).map_err(native::account)?;
            self.screen_open_scoped(target, Some(original)).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (organization, target);
            Err(ScreenFailure::Unsupported)
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(target_os = "linux")]
    async fn screen_open_scoped(
        &self,
        target: String,
        context: Option<super::organizations::OrganizationObservation>,
    ) -> Result<ScreenView, ScreenFailure> {
        let (state, life) = self.screen_device(true, context).await?;
        let life = life.ok_or(ScreenFailure::Closed)?;
        // This is a role switch only: stop/join old child, keep endpoint/owner.
        let mut slot = state.device.lock().await;
        if life.stop.is_cancelled() {
            return Err(ScreenFailure::Closed);
        }
        let device = slot.as_mut().ok_or(ScreenFailure::Closed)?;
        device
            .close_child()
            .await
            .inspect_err(|_| life.stop.cancel())?;
        let (life, viewer) = device.open(&state, life, target).await?;
        let error = {
            match self.screen_publication(&state, Some(&life)) {
                Ok(_publication) => {
                    return Ok(ScreenView {
                        state: state.clone(),
                        life,
                        viewer,
                    });
                }
                Err(error) => error,
            }
        };
        device.close_child_if(&life).await?;
        Err(error)
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(target_os = "linux")]
    fn screen_publication<'a>(
        &'a self,
        state: &Arc<State>,
        role: Option<&Arc<RoleLife>>,
    ) -> Result<std::sync::MutexGuard<'a, ()>, ScreenFailure> {
        let publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let fence = state
            .fence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ScreenFailure::Closed)?;
        self.shared_identity()
            .check_local_use_lifetime(&fence)
            .map_err(native::account)?;
        if let Some(role) = role {
            role.context.check().map_err(native::account)?;
        }
        if state.stop.is_cancelled()
            || !self.catalog_active()
            || !self
                .screen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .is_some_and(|slot| Arc::ptr_eq(&slot.0, state))
            || role.is_some_and(|role| {
                role.stop.is_cancelled()
                    || !state
                        .role
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, role))
            })
        {
            return Err(ScreenFailure::Closed);
        }
        Ok(publication)
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(target_os = "linux")]
    async fn screen_device(
        &self,
        reserve_role: bool,
        context: Option<super::organizations::OrganizationObservation>,
    ) -> Result<(Arc<State>, Option<Arc<RoleLife>>), ScreenFailure> {
        let identity = self.shared_identity().clone();
        let lifetime = identity.local_use_lifetime();
        let context = if reserve_role {
            Some(match context {
                Some(original) => original,
                None => self.observe_personal().map_err(native::account)?,
            })
        } else {
            None
        };
        let module = Arc::new(
            self.module_access("rds", super::module_control::Adapter::Shared)
                .await
                .map_err(native::account)?,
        );
        // Reap only an already finished original task. New admission never
        // queues behind a running or unresolved previous device.
        let prior = self
            .screen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|slot| slot.0.clone());
        if let Some(prior) = prior.filter(|state| state.stop.is_cancelled()) {
            let finished = prior
                .task
                .try_lock()
                .is_ok_and(|task| task.as_ref().is_none_or(|task| task.is_finished()));
            if finished {
                let _ = prior.join().await;
            }
        }
        let (state, opening, role) = {
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            identity
                .check_captured_local_use_lifetime(&lifetime)
                .map_err(native::account)?;
            if !self.catalog_active() {
                return Err(ScreenFailure::Denied);
            }
            let mut slot = self.screen.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(current) = slot.as_ref().filter(|slot| slot.0.stop.is_cancelled()) {
                match *current.0.completion.borrow() {
                    None => return Err(ScreenFailure::Busy),
                    Some(Err(error)) if !current.0.is_drained() => return Err(error),
                    Some(_) if !current.0.is_drained() => return Err(ScreenFailure::Busy),
                    Some(_) => {}
                }
            }
            let (state, opening) =
                if let Some(current) = slot.as_ref().filter(|slot| !slot.0.stop.is_cancelled()) {
                    if current
                        .0
                        .device
                        .try_lock()
                        .map_err(|_| ScreenFailure::Busy)?
                        .is_none()
                    {
                        return Err(ScreenFailure::Busy);
                    }
                    (current.0.clone(), None)
                } else {
                    slot.take();
                    let state = Arc::new(State {
                        stop: module.lifetime.child_token(),
                        module: module.clone(),
                        publication: self.publication.clone(),
                        completion: tokio::sync::watch::channel(None).0,
                        task: tokio::sync::Mutex::default(),
                        joined: std::sync::atomic::AtomicBool::new(false),
                        drained: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        hold: Mutex::default(),
                        role: Mutex::default(),
                        device: tokio::sync::Mutex::default(),
                        fence: Mutex::default(),
                    });
                    let (ready, opening) = tokio::sync::oneshot::channel();
                    let owned = state.clone();
                    let identity = identity.clone();
                    let retained = state.clone();
                    let completion = DeviceCompletion {
                        stop: state.stop.clone(),
                        result: state.completion.clone(),
                        drained: state.drained.clone(),
                        retain: Some(Box::new(move || retained.retain_unresolved())),
                        finished: false,
                    };
                    let task = tokio::spawn(
                        completion
                            .run(native::own_device(identity, lifetime, owned, ready))
                            .in_current_span()
                            .with_current_subscriber(),
                    );
                    *state.task.try_lock().expect("new screen task slot") = Some(task);
                    *slot = Some(ScreenSlot(state.clone()));
                    (state, Some(opening))
                };
            let role = context.map(|context| native::role(&state, context));
            (state, opening, role)
        };
        let result = async {
            if let Some(opening) = opening {
                opening.await.map_err(|_| ScreenFailure::Closed)??;
            } else if state
                .device
                .try_lock()
                .map_err(|_| ScreenFailure::Busy)?
                .is_none()
            {
                return Err(ScreenFailure::Busy);
            }
            let fence = state
                .fence
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .ok_or(ScreenFailure::Closed)?;
            identity
                .validate_local_use(&fence)
                .await
                .map_err(native::account)?;
            module.validate().await.map_err(native::account)?;
            let _publication = self.screen_publication(&state, role.as_ref())?;
            Ok(())
        }
        .await;
        module.observe(&result.as_ref().map(|_| ()).map_err(|error| {
            use nddev_device_sync_domain::modules::ModuleError;
            match error {
                ScreenFailure::Busy => ModuleError::Busy,
                ScreenFailure::Timeout => ModuleError::Timeout,
                ScreenFailure::Closed => ModuleError::Cancelled,
                _ => ModuleError::Unavailable,
            }
        }));
        if let Err(error) = result {
            if let Some(role) = &role {
                role.stop.cancel();
            }
            return Err(error);
        }
        Ok((state, role))
    }
}

#[cfg(target_os = "linux")]
mod native {
    use super::*;
    use nddev_device_sync_accounts::{
        ClientError, IdentityClient, LocalUseFence,
        screen::{RdsEndpointKey, ScreenArtifact, ScreenCreate},
        screen_wire,
    };
    use nddev_device_sync_application::{lifecycle::ClientLifecycle, screen::HostAccess};
    use nddev_device_sync_domain::{
        DeviceId, Platform,
        modules::ModuleError,
        screen::{
            EndpointBinding, MAX_DISPLAYS, MAX_VIEW_MS, ScreenError, ViewProjection, ViewState,
        },
    };
    use nddev_device_sync_rds::owned::{HostAdmission, OwnedEndpoint, runtime::NativeViewer};
    use rds_net::{EndpointAddr, EndpointConfig, EndpointId, SecretKey};
    use std::time::Duration;
    pub(super) fn failure(error: ScreenError) -> ScreenFailure {
        match error {
            ScreenError::Denied => ScreenFailure::Denied,
            ScreenError::Expired => ScreenFailure::Expired,
            ScreenError::Closed => ScreenFailure::Closed,
            ScreenError::FrameLimit => ScreenFailure::Limit,
            ScreenError::Module(error) => match error {
                ModuleError::UnsupportedPlatform => ScreenFailure::Unsupported,
                ModuleError::Busy => ScreenFailure::Busy,
                ModuleError::InvalidInput => ScreenFailure::InvalidInput,
                ModuleError::InvalidResponse | ModuleError::InvalidContract => {
                    ScreenFailure::InvalidResponse
                }
                ModuleError::Timeout => ScreenFailure::Timeout,
                ModuleError::Cancelled => ScreenFailure::Closed,
                ModuleError::Unavailable => ScreenFailure::Unavailable,
                ModuleError::ProviderFailed => ScreenFailure::Failed,
            },
        }
    }
    pub(super) fn account(error: ClientError) -> ScreenFailure {
        match error {
            ClientError::Busy => ScreenFailure::Busy,
            ClientError::Expired => ScreenFailure::Expired,
            ClientError::Cancelled => ScreenFailure::Closed,
            ClientError::InvalidInput | ClientError::InvalidEndpoint => ScreenFailure::InvalidInput,
            ClientError::InvalidResponse => ScreenFailure::InvalidResponse,
            ClientError::Timeout => ScreenFailure::Timeout,
            ClientError::Transport
            | ClientError::OnlineVerificationRequired
            | ClientError::CredentialStore
            | ClientError::TelemetryUnavailable => ScreenFailure::Unavailable,
            ClientError::RevalidationRequired
            | ClientError::DeviceRevoked
            | ClientError::Server(_) => ScreenFailure::Denied,
        }
    }
    fn wall_now() -> Result<u64, ScreenError> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|time| time.as_millis().try_into().ok())
            .ok_or(ScreenError::Module(ModuleError::Unavailable))
    }
    fn check(state: &State, deadline: tokio::time::Instant) -> Result<(), ScreenFailure> {
        if state.stop.is_cancelled() {
            Err(ScreenFailure::Closed)
        } else if tokio::time::Instant::now() >= deadline {
            Err(ScreenFailure::Expired)
        } else {
            Ok(())
        }
    }
    fn classify(error: &ScreenFailure) -> (String, &'static str, bool) {
        (
            error.code().into(),
            "error",
            matches!(error, ScreenFailure::Unavailable | ScreenFailure::Failed),
        )
    }
    pub(super) struct DeviceScreen {
        pub(super) identity: Arc<IdentityClient>,
        stop: CancellationToken,
        owner: Option<RdsEndpointKey>,
        pub(super) fence: Arc<LocalUseFence>,
        endpoint: Option<OwnedEndpoint>,
        binding: EndpointBinding,
        child: Option<Child>,
        drain_error: Option<ScreenFailure>,
        drained: bool,
        pub(super) pending_host: Option<PendingHost>,
        pending_view: Option<ViewProjection>,
    }
    enum Child {
        Host {
            id: String,
            life: Arc<RoleLife>,
            task: HostCompletion,
            #[cfg(test)]
            fail_after_join: Option<Arc<std::sync::atomic::AtomicBool>>,
        },
        Viewer {
            id: String,
            life: Arc<RoleLife>,
            viewer: Arc<tokio::sync::Mutex<Option<NativeViewer>>>,
        },
    }
    enum HostCompletion {
        Running(tokio::task::JoinHandle<Result<(), ScreenFailure>>),
        Known(Result<(), ScreenFailure>),
        Unresolved,
    }
    impl HostCompletion {
        async fn join(&mut self) -> Result<(), ScreenFailure> {
            if let Self::Running(task) = self {
                // Replace a consumed handle before the next await. A later
                // closer observes this exact terminal state, never polls it twice.
                *self = match (&mut *task).await {
                    Ok(result) => Self::Known(result),
                    Err(_) => Self::Unresolved,
                };
            }
            match self {
                Self::Known(result) => *result,
                Self::Unresolved | Self::Running(_) => Err(ScreenFailure::Unresolved),
            }
        }
        fn unresolved(&self) -> bool {
            matches!(self, Self::Unresolved)
        }
    }
    impl Child {
        fn life(&self) -> &Arc<RoleLife> {
            match self {
                Self::Host { life, .. } | Self::Viewer { life, .. } => life,
            }
        }
    }
    pub(super) struct PendingHost {
        pub(super) request: ViewProjection,
        admission: HostAdmission,
        artifact: ScreenArtifact,
        access: HostAccess,
        caps: rds_core::DesktopCaps,
    }
    pub(super) fn role(
        state: &State,
        context: super::super::organizations::OrganizationObservation,
    ) -> Arc<RoleLife> {
        state.cancel_role();
        let life = Arc::new(RoleLife {
            stop: state.stop.child_token(),
            validation: Mutex::default(),
            done: CancellationToken::new(),
            terminal: Mutex::default(),
            context,
        });
        *state.role.lock().unwrap_or_else(|e| e.into_inner()) = Some(life.clone());
        life
    }
    fn permitted(role: &RoleLife) -> Result<(), ScreenFailure> {
        role.context.check().map_err(account)?;
        if role.stop.is_cancelled() {
            Err(ScreenFailure::Closed)
        } else {
            Ok(())
        }
    }
    impl DeviceScreen {
        #[cfg(test)]
        pub(super) fn fail_after_join_for_fixture(
            &mut self,
            original: &Arc<RoleLife>,
            marker: Arc<std::sync::atomic::AtomicBool>,
        ) {
            match self.child.as_mut() {
                Some(Child::Host {
                    life,
                    fail_after_join,
                    ..
                }) if Arc::ptr_eq(original, life) => {
                    assert!(fail_after_join.is_none());
                    *fail_after_join = Some(marker);
                }
                _ => panic!("fixture must retain its exact actual host"),
            }
        }
        async fn prepare(
            identity: Arc<IdentityClient>,
            lifetime: &nddev_device_sync_accounts::LocalUseLifetime,
            state: &State,
        ) -> Result<Self, ScreenFailure> {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
            check(state, deadline)?;
            let fence = Arc::new(
                identity
                    .local_use_fence_from(lifetime)
                    .await
                    .map_err(account)?,
            );
            let owner = identity.rds_endpoint_key().await.map_err(account)?;
            let base = identity
                .screen_binding(&fence, owner.device())
                .await
                .map_err(account)?
                .map_or(0, |binding| binding.revision);
            let material = identity.rds_key_material(&owner).await.map_err(account)?;
            // Never abort this bind: keep owner until its exact endpoint can close.
            let mut endpoint = OwnedEndpoint::bind(
                owner.device().clone(),
                owner.instance_id(),
                SecretKey::from_bytes(&material),
                EndpointConfig::default(),
            )
            .await
            .map_err(failure)?;
            drop(material);
            let registering = async {
                check(state, deadline)?;
                identity.validate_local_use(&fence).await.map_err(account)?;
                let mut attempts = 0;
                let binding = loop {
                    check(state, deadline)?;
                    match identity.screen_bind(&owner, base).await {
                        Ok(binding) => break binding,
                        Err(ClientError::Transport | ClientError::Timeout) if attempts < 2 => {
                            attempts += 1;
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        Err(error) => return Err(account(error)),
                    }
                };
                identity.validate_local_use(&fence).await.map_err(account)?;
                check(state, deadline)?;
                endpoint
                    .accept_binding_from_accounts(binding.clone(), base)
                    .map_err(failure)?;
                Ok(binding)
            };
            let binding = match registering.await {
                Ok(binding) => binding,
                Err(error) => {
                    endpoint.close().await;
                    drop(owner);
                    return Err(error);
                }
            };
            *state.fence.lock().unwrap_or_else(|e| e.into_inner()) = Some(fence.clone());
            Ok(Self {
                identity,
                stop: state.stop.clone(),
                owner: Some(owner),
                fence,
                endpoint: Some(endpoint),
                binding,
                child: None,
                drain_error: None,
                drained: false,
                pending_host: None,
                pending_view: None,
            })
        }
        pub(super) async fn close_child_if(
            &mut self,
            role: &Arc<RoleLife>,
        ) -> Result<(), ScreenFailure> {
            if let Some(result) = *role.terminal.lock().unwrap_or_else(|e| e.into_inner()) {
                return result;
            }
            if self
                .child
                .as_ref()
                .is_some_and(|child| Arc::ptr_eq(child.life(), role))
            {
                return self.close_child().await;
            }
            Ok(())
        }
        async fn end(&self, id: &str) {
            let _ = tokio::time::timeout(
                Duration::from_secs(3),
                self.identity.screen_end(&self.fence, id),
            )
            .await;
        }
        pub(super) async fn close_active(&mut self) -> Result<(), ScreenFailure> {
            let result = super::super::identity::observe_cleanup(
                "rds",
                "rds.role.close",
                async {
                    tokio::time::timeout(Duration::from_secs(3), self.drain_active())
                        .await
                        .unwrap_or(Err(ScreenFailure::Timeout))
                },
                classify_close,
            )
            .await;
            if result.is_err() {
                self.stop.cancel();
            }
            result
        }
        async fn drain_active(&mut self) -> Result<(), ScreenFailure> {
            let Some(child) = self.child.as_mut() else {
                return self.drain_error.map_or(Ok(()), Err);
            };
            #[cfg(test)]
            let fail_after_join = match child {
                Child::Host {
                    fail_after_join, ..
                } => fail_after_join.clone(),
                Child::Viewer { .. } => None,
            };
            let (id, life, result) = match child {
                Child::Host { id, life, task, .. } => {
                    life.stop.cancel();
                    let result = super::super::identity::observe_cleanup(
                        "rds",
                        "rds.host.join",
                        task.join(),
                        classify_close,
                    )
                    .await;
                    if task.unresolved() {
                        self.stop.cancel();
                        return Err(ScreenFailure::Unresolved);
                    }
                    let result = match result {
                        Ok(()) | Err(ScreenFailure::Closed | ScreenFailure::Expired) => Ok(()),
                        error => error,
                    };
                    #[cfg(test)]
                    let result = if result.is_ok()
                        && let Some(marker) = fail_after_join
                    {
                        marker.store(true, std::sync::atomic::Ordering::Release);
                        Err(ScreenFailure::Failed)
                    } else {
                        result
                    };
                    (id.clone(), life.clone(), result)
                }
                Child::Viewer { id, life, viewer } => {
                    life.stop.cancel();
                    let mut slot = viewer.lock().await;
                    let result = if let Some(viewer) = slot.as_mut() {
                        let result = super::super::identity::observe_cleanup(
                            "rds",
                            "rds.viewer.close",
                            async { viewer.close().await.map_err(failure) },
                            classify_close,
                        )
                        .await;
                        if !viewer.is_drained() {
                            return Err(ScreenFailure::Unresolved);
                        }
                        result
                    } else {
                        Ok(())
                    };
                    slot.take();
                    (id.clone(), life.clone(), result)
                }
            };
            // A later empty drain must not erase an already observed failure.
            if self.drain_error.is_none() {
                self.drain_error = result.err();
            }
            self.child.take();
            let result = self.drain_error.map_or(Ok(()), Err);
            // Native completion and its exact result are retained before the
            // remote best-effort end await can be cancelled by a closer.
            life.finish(result);
            self.end(&id).await;
            result
        }
        pub(super) async fn close_child(&mut self) -> Result<(), ScreenFailure> {
            self.close_active().await?;
            if let Some(pending) = self.pending_host.take() {
                self.end(pending.request.id()).await;
            }
            if let Some(pending) = self.pending_view.take() {
                self.end(pending.id()).await;
            }
            // Endpoint/key/UUID/binding remain unchanged for the next admitted role.
            Ok(())
        }
        async fn close(&mut self) -> Result<(), ScreenFailure> {
            // Keep cleanup unconditional and return its exact terminal result.
            let closing = async {
                let result = self.drain_active().await;
                // A native host/viewer without a positive drain witness keeps
                // its endpoint, original purpose owner and role in this device.
                if self.child.is_some() {
                    return Err(ScreenFailure::Unresolved);
                }
                if let Some(pending) = self.pending_host.take() {
                    self.end(pending.request.id()).await;
                }
                if let Some(pending) = self.pending_view.take() {
                    self.end(pending.id()).await;
                }
                if !self.drained {
                    let endpoint = self.endpoint.take().ok_or(ScreenFailure::Unresolved)?;
                    endpoint.close().await;
                    self.owner.take();
                    self.drained = true;
                }
                result
            };
            super::super::identity::observe_cleanup(
                "rds",
                "rds.device.close",
                closing,
                classify_close,
            )
            .await
        }
        pub(super) async fn open(
            &mut self,
            state: &State,
            life: Arc<RoleLife>,
            target: String,
        ) -> Result<(Arc<RoleLife>, Arc<tokio::sync::Mutex<Option<NativeViewer>>>), ScreenFailure>
        {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
            let opening = async {
                let target = DeviceId::new(target).map_err(|_| ScreenFailure::InvalidInput)?;
                check(state, deadline)?;
                permitted(&life)?;
                self.identity
                    .validate_local_use(&self.fence)
                    .await
                    .map_err(account)?;
                let host = self
                    .identity
                    .screen_binding(&self.fence, &target)
                    .await
                    .map_err(account)?
                    .ok_or(ScreenFailure::Unavailable)?;
                let expires = wall_now()
                    .map_err(failure)?
                    .checked_add(MAX_VIEW_MS)
                    .ok_or(ScreenFailure::InvalidInput)?
                    / 1000
                    * 1000;
                let create = ScreenCreate::new(screen_wire::ScreenViewRequest {
                    target_device_id: target.as_str().into(),
                    viewer_revision: self.binding.revision as i64,
                    host_revision: host.revision as i64,
                    display: 0,
                    max_fps: 30,
                    output_height: 1080,
                    expires_at_ms: expires as i64,
                    idempotency_key: uuid::Uuid::new_v4().to_string(),
                })
                .map_err(account)?;
                let mut attempts = 0;
                let original = loop {
                    check(state, deadline)?;
                    permitted(&life)?;
                    match self.identity.screen_create(&self.fence, &create).await {
                        Ok(view) => break view,
                        Err(ClientError::Transport | ClientError::Timeout) if attempts < 2 => {
                            attempts += 1;
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        Err(error) => return Err(account(error)),
                    }
                };
                let id = original.id().to_owned();
                self.pending_view = Some(original);
                let delivery = loop {
                    check(state, deadline)?;
                    permitted(&life)?;
                    let mut delivery = self
                        .identity
                        .screen_view(&self.fence, &id)
                        .await
                        .map_err(account)?;
                    match delivery.view.state(wall_now().map_err(failure)?) {
                        ViewState::Ready => break delivery,
                        ViewState::Ended => return Err(ScreenFailure::Closed),
                        _ => tokio::time::sleep(Duration::from_secs(1)).await,
                    }
                };
                let mut view = delivery.view;
                let artifact = delivery.artifact.ok_or(ScreenFailure::InvalidResponse)?;
                self.identity
                    .validate_local_use(&self.fence)
                    .await
                    .map_err(account)?;
                check(state, deadline)?;
                permitted(&life)?;
                let target = EndpointAddr::new(
                    EndpointId::from_bytes(&host.endpoint)
                        .map_err(|_| ScreenFailure::InvalidResponse)?,
                );
                let live = self
                    .endpoint
                    .as_ref()
                    .ok_or(ScreenFailure::Closed)?
                    .open_view(&mut view, target, artifact.bytes(), life.stop.clone())
                    .await
                    .map_err(failure)?;
                let viewer = Arc::new(tokio::sync::Mutex::new(Some(live)));
                self.child = Some(Child::Viewer {
                    id,
                    life: life.clone(),
                    viewer: viewer.clone(),
                });
                self.pending_view.take();
                // Keep the handle owned for teardown; Shared guards its publication.
                self.identity
                    .validate_local_use(&self.fence)
                    .await
                    .map_err(account)?;
                check(state, deadline)?;
                permitted(&life)?;
                *life.validation.lock().unwrap_or_else(|e| e.into_inner()) = Some(view);
                Ok(viewer)
            };
            let result = super::super::identity::observe_operation(
                "rds",
                "rds.viewer.open",
                opening,
                ScreenFailure::Unavailable,
                classify,
            )
            .await;
            match result {
                Ok(viewer) => Ok((life, viewer)),
                Err(error) => {
                    life.stop.cancel();
                    let cleanup = self.close_child().await;
                    if self
                        .child
                        .as_ref()
                        .is_some_and(|child| Arc::ptr_eq(child.life(), &life))
                    {
                        return Err(ScreenFailure::Unresolved);
                    }
                    life.finish(cleanup);
                    Err(error)
                }
            }
        }
        pub(super) async fn accept(
            &mut self,
            state: &State,
            life: Arc<RoleLife>,
            id: &str,
            raw: nddev_device_sync_telemetry::view::ClientLifecycle,
        ) -> Result<Arc<RoleLife>, ScreenFailure> {
            let lifecycle = match raw {
                nddev_device_sync_telemetry::view::ClientLifecycle::Resumed => {
                    ClientLifecycle::Resumed
                }
                nddev_device_sync_telemetry::view::ClientLifecycle::Inactive => {
                    ClientLifecycle::Inactive
                }
                nddev_device_sync_telemetry::view::ClientLifecycle::Hidden => {
                    ClientLifecycle::Hidden
                }
                nddev_device_sync_telemetry::view::ClientLifecycle::Paused => {
                    ClientLifecycle::Paused
                }
                nddev_device_sync_telemetry::view::ClientLifecycle::Detached => {
                    ClientLifecycle::Detached
                }
                nddev_device_sync_telemetry::view::ClientLifecycle::Ready => {
                    return Err(ScreenFailure::Denied);
                }
            };
            let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
            let accepting = async {
                self.identity
                    .validate_local_use(&self.fence)
                    .await
                    .map_err(account)?;
                check(state, deadline)?;
                permitted(&life)?;
                if self
                    .pending_host
                    .as_ref()
                    .is_some_and(|pending| pending.request.id() != id)
                {
                    return Err(ScreenFailure::Busy);
                }
                if self.pending_host.is_none() {
                    let mut request = self
                        .identity
                        .screen_incoming(&self.fence)
                        .await
                        .map_err(account)?
                        .ok_or(ScreenFailure::Closed)?;
                    if request.id() != id
                        || request.state(wall_now().map_err(failure)?) != ViewState::Requested
                    {
                        return Err(ScreenFailure::Closed);
                    }
                    // Real bounded native inventory, not metadata/source override.
                    let caps = rds_desktop::capabilities_bounded()
                        .await
                        .map_err(|_| ScreenFailure::Unavailable)?;
                    let display = request.permit().lease.display();
                    if caps.displays.len() > MAX_DISPLAYS
                        || !caps.codecs.contains(&rds_core::Codec::H264)
                        || !caps.displays.iter().any(|source| source.index == display)
                    {
                        return Err(ScreenFailure::Unavailable);
                    }
                    self.identity
                        .validate_local_use(&self.fence)
                        .await
                        .map_err(account)?;
                    check(state, deadline)?;
                    permitted(&life)?;
                    let access = HostAccess {
                        local_device: self
                            .owner
                            .as_ref()
                            .ok_or(ScreenFailure::Closed)?
                            .device()
                            .clone(),
                        local_endpoint: self
                            .endpoint
                            .as_ref()
                            .ok_or(ScreenFailure::Closed)?
                            .public_key(),
                        local_binding_revision: self.binding.revision,
                        local_opt_in: true, // This exact explicit accept action, not discovery metadata.
                        source_ready: true, // Actual bounded capabilities above; capture can still fail closed.
                        display,
                        source_expires_at_ms: request.permit().lease.expires_at_ms(),
                        lifecycle,
                        platform: Platform::Linux,
                    };
                    let admission = self
                        .endpoint
                        .as_ref()
                        .ok_or(ScreenFailure::Closed)?
                        .admit_host(&mut request, &access, wall_now().map_err(failure)?)
                        .map_err(failure)?;
                    let artifact =
                        ScreenArtifact::new(admission.native_artifact()).map_err(account)?;
                    self.pending_host = Some(PendingHost {
                        request,
                        admission,
                        artifact,
                        access,
                        caps,
                    });
                }
                let pending = self.pending_host.as_ref().ok_or(ScreenFailure::Closed)?;
                let mut attempts = 0;
                let ready = loop {
                    check(state, deadline)?;
                    permitted(&life)?;
                    match self
                        .identity
                        .screen_ready(&self.fence, &pending.request, &pending.artifact)
                        .await
                    {
                        Ok(view) => break view,
                        Err(ClientError::Transport | ClientError::Timeout) if attempts < 2 => {
                            attempts += 1;
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        Err(error @ (ClientError::Transport | ClientError::Timeout)) => {
                            let error = account(error);
                            tracing::event!(target:"nds_native",tracing::Level::WARN,{
                                event.name="rds.host.ready.unconfirmed",module="rds",scope="process",
                                error.type=error.code(),outcome="pending"
                            });
                            return Err(error);
                        }
                        Err(error) => return Err(account(error)),
                    }
                };
                self.identity
                    .validate_local_use(&self.fence)
                    .await
                    .map_err(account)?;
                check(state, deadline)?;
                permitted(&life)?;
                let pending = self.pending_host.take().ok_or(ScreenFailure::Closed)?;
                let control_view = ready.clone();
                let host =
                    match pending
                        .admission
                        .make_builtin_x11(ready, &pending.access, pending.caps)
                    {
                        Ok(host) => host,
                        Err(error) => {
                            self.end(id).await;
                            return Err(failure(error));
                        }
                    };
                let owned = life.clone();
                let task = tokio::spawn(
                    async move {
                        let result = super::super::identity::observe_cleanup(
                            "rds",
                            "rds.host.stop",
                            async { host.serve(owned.stop.clone()).await.map_err(failure) },
                            classify_close,
                        )
                        .await;
                        owned.stop.cancel();
                        result
                    }
                    .in_current_span()
                    .with_current_subscriber(),
                );
                self.child = Some(Child::Host {
                    id: id.into(),
                    life: life.clone(),
                    task: HostCompletion::Running(task),
                    #[cfg(test)]
                    fail_after_join: None,
                });
                *life.validation.lock().unwrap_or_else(|e| e.into_inner()) = Some(control_view);
                // NdsClient guards the final handle publication after this result.
                Ok(())
            };
            let result = super::super::identity::observe_operation(
                "rds",
                "rds.host.accept",
                accepting,
                ScreenFailure::Unavailable,
                classify,
            )
            .await;
            match result {
                Ok(()) => Ok(life),
                Err(error) => {
                    life.stop.cancel();
                    // Uncertain ready retains the same pending request/artifact for
                    // explicit retry. No source task is started before a Ready ACK.
                    life.finish(Ok(()));
                    Err(error)
                }
            }
        }
    }
    pub(super) async fn own_device(
        identity: Arc<IdentityClient>,
        lifetime: nddev_device_sync_accounts::LocalUseLifetime,
        state: Arc<State>,
        ready: tokio::sync::oneshot::Sender<Result<(), ScreenFailure>>,
    ) -> Result<(), ScreenFailure> {
        let result = super::super::identity::observe_operation(
            "rds",
            "rds.device.prepare",
            DeviceScreen::prepare(identity, &lifetime, &state),
            ScreenFailure::Unavailable,
            classify,
        )
        .await;
        let control = match result {
            Ok(device) => {
                let control = (device.identity.clone(), device.fence.clone());
                *state.device.lock().await = Some(device);
                let _ = ready.send(Ok(()));
                control
            }
            Err(error) => {
                state.stop.cancel();
                state
                    .drained
                    .store(true, std::sync::atomic::Ordering::Release);
                let _ = ready.send(Err(error));
                return Err(error);
            }
        };
        // The existing device owner also owns remote permission checks: no
        // detached watcher or new task registry. Idle registrations do no I/O.
        // Each active role checks every five seconds, with three seconds for a
        // fresh answer. Failure cancels media; neither retry nor success renews it.
        loop {
            tokio::select! {
                _ = state.stop.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
            let local_consent = async {
                for attempt in 0..8 {
                    match state.module.validate().await {
                        Ok(()) => return Ok(()),
                        Err(ClientError::Busy) if attempt < 7 => {
                            tokio::time::sleep(Duration::from_millis((25u64 << attempt).min(400)))
                                .await
                        }
                        Err(error) => return Err(account(error)),
                    }
                }
                Err(ScreenFailure::Busy)
            };
            if !matches!(
                tokio::time::timeout(Duration::from_secs(3), local_consent).await,
                Ok(Ok(()))
            ) {
                state.stop.cancel();
                break;
            }
            let role = state.role.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let Some(role) = role else { continue };
            if role.done.is_cancelled() {
                continue;
            }
            if role.context.check().is_err() {
                role.stop.cancel();
            }
            let original = role
                .validation
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if let Some(original) = original.filter(|_| !role.stop.is_cancelled()) {
                let revalidate = async {
                    for attempt in 0..8 {
                        match control.0.screen_revalidate(&control.1, &original).await {
                            Ok(true) => return Ok(()),
                            Ok(false) => return Err(ScreenFailure::Closed),
                            Err(ClientError::Busy) if attempt < 7 => {
                                tokio::time::sleep(Duration::from_millis(
                                    (25u64 << attempt).min(400),
                                ))
                                .await;
                            }
                            Err(error) => return Err(account(error)),
                        }
                    }
                    Err(ScreenFailure::Busy)
                };
                let result = tokio::select! {
                    _ = state.stop.cancelled() => break,
                    _ = role.stop.cancelled() => Err(ScreenFailure::Closed),
                    result = super::super::identity::observe_operation(
                        "rds", "rds.view.revalidate",
                        async { tokio::time::timeout(Duration::from_secs(3), revalidate)
                            .await.unwrap_or(Err(ScreenFailure::Timeout)) },
                        ScreenFailure::Unavailable, classify,
                    ) => result,
                };
                if result.is_err() {
                    role.stop.cancel();
                }
            }
            if role.stop.is_cancelled() && !role.done.is_cancelled() {
                // Close only this role: a newer opening cannot be canceled by an
                // earlier response. Cancellation reaches native media immediately.
                let closed =
                    tokio::time::timeout(Duration::from_secs(3), state.close_role(&role)).await;
                if !matches!(closed, Ok(Ok(()))) {
                    state.stop.cancel();
                }
            }
        }
        // The device stays in its registered owner across every close await.
        // Host tasks own only their host/life and never lock this slot.
        let mut slot = state.device.lock().await;
        let result = match slot.as_mut() {
            Some(device) => device.close().await,
            None => Ok(()),
        };
        if slot.as_ref().is_none_or(|device| device.drained) {
            slot.take();
            state
                .drained
                .store(true, std::sync::atomic::Ordering::Release);
        }
        result
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn known_host_failure_survives_role_close_and_device_completion() {
            let client = NdsClient::new("https://example.invalid".into()).unwrap();
            let life = RoleLife {
                stop: CancellationToken::new(),
                validation: Mutex::default(),
                done: CancellationToken::new(),
                terminal: Mutex::default(),
                context: client.observe_personal().unwrap(),
            };
            let mut task =
                HostCompletion::Running(tokio::spawn(async { Err(ScreenFailure::Failed) }));
            let result = super::super::super::identity::observe_cleanup(
                "rds",
                "rds.host.join",
                task.join(),
                classify_close,
            )
            .await;
            life.finish(result);
            life.finish(Ok(()));
            let (sender, first) = tokio::sync::watch::channel(None);
            let second = first.clone();
            let completed = *life.terminal.lock().unwrap();
            super::super::completion_tests::guard(&life.stop, sender.clone(), true)
                .run(async { completed.unwrap() })
                .await;
            let repeated_join = task.join().await;
            assert_eq!(result, Err(ScreenFailure::Failed));
            assert_eq!(repeated_join, result);
            assert!(!task.unresolved());
            assert!(life.done.is_cancelled());
            assert_eq!(*first.borrow(), Some(result));
            assert_eq!(*second.borrow(), Some(result));
            assert_eq!(*sender.subscribe().borrow(), Some(result));
        }

        #[tokio::test]
        async fn a_cancelled_join_borrows_the_original_host_handle() {
            let (release, held) = tokio::sync::oneshot::channel();
            let mut task = HostCompletion::Running(tokio::spawn(async move {
                let _ = held.await;
                Err(ScreenFailure::Failed)
            }));
            let interrupted = tokio::time::timeout(Duration::from_millis(5), task.join()).await;
            let still_owned = matches!(task, HostCompletion::Running(_));
            let _ = release.send(());
            let joined = task.join().await;
            let repeated = task.join().await;
            assert!(interrupted.is_err());
            assert!(still_owned);
            assert_eq!(joined, Err(ScreenFailure::Failed));
            assert_eq!(repeated, joined);
        }

        #[tokio::test]
        async fn a_lost_host_result_is_sticky_unresolved_not_a_second_handle_poll() {
            let mut task = HostCompletion::Running(tokio::spawn(async {
                panic!("inert host result lost");
            }));
            let first = task.join().await;
            let second = task.join().await;
            assert_eq!(first, Err(ScreenFailure::Unresolved));
            assert_eq!(second, first);
            assert!(task.unresolved());
        }
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    pub(super) fn guard(
        stop: &CancellationToken,
        result: tokio::sync::watch::Sender<Option<Result<(), ScreenFailure>>>,
        drained: bool,
    ) -> DeviceCompletion {
        DeviceCompletion {
            stop: stop.clone(),
            result,
            drained: Arc::new(std::sync::atomic::AtomicBool::new(drained)),
            retain: None,
            finished: false,
        }
    }
    #[test]
    fn completion_is_shared_and_drop_never_claims_success() {
        let stop = CancellationToken::new();
        let (sender, first) = tokio::sync::watch::channel(None);
        let second = first.clone();
        drop(guard(&stop, sender, false));
        assert!(stop.is_cancelled());
        assert_eq!(*first.borrow(), Some(Err(ScreenFailure::Unresolved)));
        assert_eq!(*second.borrow(), Some(Err(ScreenFailure::Unresolved)));

        let stop = CancellationToken::new();
        let (sender, first) = tokio::sync::watch::channel(None);
        let second = first.clone();
        guard(&stop, sender, true).finish(Ok(()));
        assert!(stop.is_cancelled());
        assert_eq!(*first.borrow(), Some(Ok(())));
        assert_eq!(*second.borrow(), Some(Ok(())));

        // The production State initially has no receiver. A waiter arriving
        // after completion must still receive the retained terminal result.
        for success in [false, true] {
            let stop = CancellationToken::new();
            let (sender, initial) = tokio::sync::watch::channel(None);
            let retained = sender.clone();
            drop(initial);
            let guard = guard(&stop, sender, success);
            if success {
                guard.finish(Ok(()));
            } else {
                drop(guard);
            }
            let late = retained.subscribe();
            assert_eq!(
                *late.borrow(),
                Some(if success {
                    Ok(())
                } else {
                    Err(ScreenFailure::Unresolved)
                })
            );
            assert!(stop.is_cancelled());
        }
    }

    #[tokio::test]
    async fn lost_outer_completion_retains_its_owner_and_never_publishes_a_drain() {
        for polled in [false, true] {
            let stop = CancellationToken::new();
            let (sender, result) = tokio::sync::watch::channel(None);
            let retained = Arc::new(Mutex::new(None));
            let purpose = Arc::new(());
            let weak = Arc::downgrade(&purpose);
            let target = retained.clone();
            let mut completion = guard(&stop, sender, false);
            completion.retain = Some(Box::new(move || *target.lock().unwrap() = Some(purpose)));
            let work = completion.run(async move {
                panic!("owned inert outer task");
            });
            if polled {
                assert!(tokio::spawn(work).await.is_err());
            } else {
                drop(work);
            }
            assert!(stop.is_cancelled());
            assert_eq!(*result.borrow(), Some(Err(ScreenFailure::Unresolved)));
            assert!(weak.upgrade().is_some());
            // Only this inert test's marker is disposed; no native owner is
            // declared drained or reclaimed by production code.
            retained.lock().unwrap().take();
            assert!(weak.upgrade().is_none());
        }
    }
}
