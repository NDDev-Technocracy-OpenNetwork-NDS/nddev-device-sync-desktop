//! The actual foreground Git connection, retained past caller loss. The GitHub module
//! client still owns Git/provider effects and its original authorization checks.
//! This slot owns task/result/drain only; it is not a durable Git-effect journal.
use super::catalog_types::{GithubCatalogError, GithubConnectionView};
use nddev_device_sync_accounts::{ClientError, runtime};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::sync::{Mutex as AsyncMutex, watch};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

type ConnectionResult = Result<GithubConnectionView, GithubCatalogError>;
type Join = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Default)]
#[cfg_attr(feature = "flutter", flutter_rust_bridge::frb(ignore))]
pub(super) struct CatalogOperations(Mutex<CatalogState>);
#[derive(Default)]
#[cfg_attr(feature = "flutter", flutter_rust_bridge::frb(ignore))]
struct CatalogState {
    generation: Arc<()>,
    row: Option<Arc<Row>>,
}
#[derive(Clone)]
#[cfg_attr(feature = "flutter", flutter_rust_bridge::frb(ignore))]
pub(super) struct Generation(Arc<()>);

#[derive(Clone)]
enum Completion {
    Pending,
    Known(Arc<ConnectionResult>),
    Unresolved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interest {
    Waiting,
    Delivered,
    Abandoned,
}
struct Row {
    // The exact CatalogClient keeps its Vault, credential/checkout guards and
    // native resources alive. An unresolved completion retains this owner.
    _owner: Arc<dyn Send + Sync>,
    admitted: CancellationToken,
    raw: Mutex<Option<Arc<ConnectionResult>>>,
    result: watch::Sender<Completion>,
    work: AsyncMutex<Option<Join>>,
    completion: AsyncMutex<Option<Join>>,
    joined: Mutex<bool>,
    interest: Mutex<Interest>,
    hold: Mutex<Option<Arc<Row>>>,
}
impl CatalogOperations {
    pub(super) fn generation(&self) -> Generation {
        Generation(
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .generation
                .clone(),
        )
    }
    /// Seal delivery/admission captured before this transition. The running
    /// future is never aborted: cancellation is the existing CatalogClient's job.
    pub(super) fn invalidate(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).generation = Arc::new(());
    }
    /// Join only an already terminal row before another explicit admission.
    /// Running work refuses immediately; caller loss never creates a queue.
    pub(super) async fn reap_completed(&self) -> Result<(), GithubCatalogError> {
        let row = self.0.lock().unwrap_or_else(|e| e.into_inner()).row.clone();
        if let Some(row) = row {
            if row.unresolved() {
                return Err(GithubCatalogError::Unavailable);
            }
            if !matches!(*row.result.borrow(), Completion::Known(_)) {
                return Err(GithubCatalogError::Busy);
            }
            row.join().await?;
        }
        Ok(())
    }
    pub(super) fn start(
        &self,
        generation: &Generation,
        owner: Arc<dyn Send + Sync>,
        work: impl Future<Output = ConnectionResult> + Send + 'static,
    ) -> Result<Waiter, GithubCatalogError> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !Arc::ptr_eq(&state.generation, &generation.0) {
            return Err(GithubCatalogError::Cancelled);
        }
        if let Some(row) = &state.row {
            if row.unresolved() {
                return Err(GithubCatalogError::Unavailable);
            }
            if !row.reclaimable() {
                return Err(GithubCatalogError::Busy);
            }
        }
        let row = Arc::new(Row {
            _owner: owner,
            admitted: CancellationToken::new(),
            raw: Mutex::default(),
            result: watch::channel(Completion::Pending).0,
            work: AsyncMutex::default(),
            completion: AsyncMutex::default(),
            joined: Mutex::new(false),
            interest: Mutex::new(Interest::Waiting),
            hold: Mutex::default(),
        });
        // Both task handles and the pre-spawn completion guard are registered
        // under the admission lock. Neither task polls the provider before it.
        row.launch(work);
        state.row = Some(row.clone());
        drop(state);
        row.admitted.cancel();
        Ok(Waiter {
            row,
            delivered: false,
        })
    }
    /// Called under the original ModuleUseGuard's publication/configuration
    /// lock, after its awaited validation. No new authorization is obtained.
    pub(super) fn publish<T>(
        &self,
        generation: &Generation,
        waiter: &mut Waiter,
        caller: impl FnOnce() -> Result<T, GithubCatalogError>,
    ) -> Result<T, GithubCatalogError> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !Arc::ptr_eq(&state.generation, &generation.0)
            || state
                .row
                .as_ref()
                .is_none_or(|row| !Arc::ptr_eq(row, &waiter.row))
            || !*waiter.row.joined.lock().unwrap_or_else(|e| e.into_inner())
            || waiter.row.unresolved()
        {
            return Err(GithubCatalogError::Cancelled);
        }
        let value = caller()?;
        *waiter
            .row
            .interest
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Interest::Delivered;
        waiter.delivered = true;
        Ok(value)
    }
    /// Known provider failures can describe effects and still be fully joined.
    /// Missing work/completion is HOLD, never a fabricated provider outcome.
    pub(super) async fn drain(&self) -> Result<(), ClientError> {
        let row = self.0.lock().unwrap_or_else(|e| e.into_inner()).row.clone();
        match row {
            Some(row) => row.join().await.map(|_| ()).map_err(|_| unresolved()),
            None => Ok(()),
        }
    }
}
fn unresolved() -> ClientError {
    ClientError::Server("github_cleanup_unresolved".into())
}
impl Row {
    fn launch(self: &Arc<Self>, work: impl Future<Output = ConnectionResult> + Send + 'static) {
        let row = self.clone();
        let span = tracing::Span::current();
        let task = runtime::spawn(
            async move {
                row.admitted.cancelled().await;
                if row.unresolved() {
                    return;
                }
                let result = Arc::new(work.await);
                *row.raw.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
            }
            .instrument(span.clone()),
        );
        *self.work.try_lock().expect("new work slot") = Some(Box::pin(task.join()));
        let row = self.clone();
        let terminal = Terminal {
            row: row.clone(),
            finished: false,
        };
        let task = runtime::spawn(
            async move {
                let terminal = terminal;
                row.admitted.cancelled().await;
                join_retained(&row.work).await;
                let result = row.raw.lock().unwrap_or_else(|e| e.into_inner()).clone();
                terminal.finish(result);
            }
            .instrument(span),
        );
        *self.completion.try_lock().expect("new completion slot") = Some(Box::pin(task.join()));
    }
    fn unresolved(&self) -> bool {
        matches!(*self.result.borrow(), Completion::Unresolved)
    }
    fn reclaimable(&self) -> bool {
        *self.joined.lock().unwrap_or_else(|e| e.into_inner())
            && !self.unresolved()
            && *self.interest.lock().unwrap_or_else(|e| e.into_inner()) != Interest::Waiting
    }
    async fn join(&self) -> Result<Arc<ConnectionResult>, GithubCatalogError> {
        // Borrow the stored join future. Dropping any caller/drain await leaves
        // that same future and handle here, rather than detaching its task.
        join_retained(&self.completion).await;
        *self.joined.lock().unwrap_or_else(|e| e.into_inner()) = true;
        match &*self.result.borrow() {
            Completion::Known(result) => Ok(result.clone()),
            Completion::Pending | Completion::Unresolved => Err(GithubCatalogError::Unavailable),
        }
    }
}
async fn join_retained(slot: &AsyncMutex<Option<Join>>) {
    let mut slot = slot.lock().await;
    if let Some(join) = slot.as_mut() {
        join.await;
        slot.take();
    }
}
struct Terminal {
    row: Arc<Row>,
    finished: bool,
}
impl Terminal {
    fn finish(mut self, result: Option<Arc<ConnectionResult>>) {
        match result {
            Some(result) => {
                self.row.result.send_replace(Completion::Known(result));
            }
            None => self.hold(),
        }
        self.finished = true;
    }
    fn hold(&self) {
        // Preserve the exact native owner on lost completion, including caller
        // disposal. This closed one-row slot is not reset by a new generation.
        *self.row.hold.lock().unwrap_or_else(|e| e.into_inner()) = Some(self.row.clone());
        self.row.result.send_replace(Completion::Unresolved);
        self.row.admitted.cancel();
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if !self.finished {
            self.hold();
        }
    }
}
pub(super) struct Waiter {
    row: Arc<Row>,
    delivered: bool,
}
impl Waiter {
    pub(super) async fn result(&self) -> Result<Arc<ConnectionResult>, GithubCatalogError> {
        self.row.join().await
    }
}
impl Drop for Waiter {
    fn drop(&mut self) {
        if !self.delivered {
            *self.row.interest.lock().unwrap_or_else(|e| e.into_inner()) = Interest::Abandoned;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    fn success() -> ConnectionResult {
        Ok(GithubConnectionView {
            outcome: "connected".into(),
            reason: "cloned".into(),
            relative_path: "owner/repository".into(),
            intent_queued: false,
        })
    }
    #[tokio::test]
    async fn completed_abandoned_work_can_be_joined_without_identity_shutdown() {
        let slot = CatalogOperations::default();
        let generation = slot.generation();
        let (send, receive) = oneshot::channel();
        let waiter = slot
            .start(&generation, Arc::new(()), async move {
                receive.await.unwrap();
                success()
            })
            .unwrap();
        let row = waiter.row.clone();
        let mut result = row.result.subscribe();
        drop(waiter);
        assert_eq!(slot.reap_completed().await, Err(GithubCatalogError::Busy));
        send.send(()).unwrap();
        runtime::timeout(std::time::Duration::from_secs(1), async {
            result
                .wait_for(|value| matches!(value, Completion::Known(_)))
                .await
                .unwrap();
        })
        .await
        .unwrap();
        assert!(!row.reclaimable());
        slot.reap_completed().await.unwrap();
        assert!(row.reclaimable());
        let next = slot
            .start(&generation, Arc::new(()), async { success() })
            .unwrap();
        next.result().await.unwrap();
    }
    #[tokio::test]
    async fn abandoned_caller_keeps_exact_work_result_and_nonqueued_admission() {
        let slot = CatalogOperations::default();
        let generation = slot.generation();
        let (send, receive) = oneshot::channel();
        let waiter = slot
            .start(&generation, Arc::new(()), async move {
                receive.await.unwrap();
                success()
            })
            .unwrap();
        let row = waiter.row.clone();
        drop(waiter);
        assert!(matches!(
            slot.start(&generation, Arc::new(()), async { success() }),
            Err(GithubCatalogError::Busy)
        ));
        slot.invalidate();
        send.send(()).unwrap();
        slot.drain().await.unwrap();
        assert_eq!(
            row.raw
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .as_ref()
                .as_ref()
                .unwrap()
                .reason,
            "cloned"
        );
        assert_eq!(*row.interest.lock().unwrap(), Interest::Abandoned);
        assert!(row.reclaimable());
        let next = slot
            .start(&slot.generation(), Arc::new(()), async { success() })
            .unwrap();
        next.result().await.unwrap();
    }
    #[tokio::test]
    async fn interrupted_drain_retains_the_same_join_and_all_waiters_see_one_result() {
        let slot = Arc::new(CatalogOperations::default());
        let (send, receive) = oneshot::channel();
        let waiter = slot
            .start(&slot.generation(), Arc::new(()), async move {
                receive.await.unwrap();
                Err(GithubCatalogError::CredentialChanged)
            })
            .unwrap();
        assert!(
            runtime::timeout(std::time::Duration::from_millis(5), slot.drain())
                .await
                .is_err()
        );
        send.send(()).unwrap();
        let (first, second) = tokio::join!(slot.drain(), slot.drain());
        first.unwrap();
        second.unwrap();
        assert!(matches!(
            waiter.result().await.unwrap().as_ref(),
            Err(GithubCatalogError::CredentialChanged)
        ));
    }
    #[tokio::test]
    async fn joined_private_effect_is_not_delivered_to_a_replacement_generation() {
        let slot = CatalogOperations::default();
        let generation = slot.generation();
        let mut waiter = slot
            .start(&generation, Arc::new(()), async { success() })
            .unwrap();
        let result = waiter.result().await.unwrap();
        slot.invalidate();
        assert_eq!(
            slot.publish(&generation, &mut waiter, || Ok(())),
            Err(GithubCatalogError::Cancelled)
        );
        assert_eq!(result.as_ref().as_ref().unwrap().outcome, "connected");
        drop(waiter);
        slot.drain().await.unwrap();
    }
    #[tokio::test]
    async fn original_caller_refusal_leaves_the_exact_joined_effect_private() {
        let slot = CatalogOperations::default();
        let generation = slot.generation();
        let mut waiter = slot
            .start(&generation, Arc::new(()), async { success() })
            .unwrap();
        let result = waiter.result().await.unwrap();
        assert_eq!(
            slot.publish(&generation, &mut waiter, || Err::<(), _>(
                GithubCatalogError::AuthorizationRequired
            )),
            Err(GithubCatalogError::AuthorizationRequired)
        );
        assert_eq!(result.as_ref().as_ref().unwrap().outcome, "connected");
        assert!(!waiter.delivered);
        assert!(!waiter.row.reclaimable());
        drop(waiter);
        slot.drain().await.unwrap();
    }
    #[tokio::test]
    async fn panic_is_unresolved_and_cannot_be_erased_by_invalidation() {
        let slot = CatalogOperations::default();
        let waiter = slot
            .start(&slot.generation(), Arc::new(()), async {
                panic!("inert work panic")
            })
            .unwrap();
        assert!(matches!(
            waiter.result().await,
            Err(GithubCatalogError::Unavailable)
        ));
        assert!(slot.drain().await.is_err());
        assert!(waiter.row.raw.lock().unwrap().is_none());
        slot.invalidate();
        assert!(matches!(
            slot.start(&slot.generation(), Arc::new(()), async { success() }),
            Err(GithubCatalogError::Unavailable)
        ));
    }
    #[tokio::test]
    async fn old_admission_does_not_poll_work_and_positive_delivery_consumes_interest() {
        let slot = CatalogOperations::default();
        let stale = slot.generation();
        slot.invalidate();
        assert!(matches!(
            slot.start(&stale, Arc::new(()), async { panic!("must not run") }),
            Err(GithubCatalogError::Cancelled)
        ));
        let generation = slot.generation();
        let mut waiter = slot
            .start(&generation, Arc::new(()), async { success() })
            .unwrap();
        waiter.result().await.unwrap();
        slot.publish(&generation, &mut waiter, || Ok(())).unwrap();
        assert_eq!(*waiter.row.interest.lock().unwrap(), Interest::Delivered);
        assert!(waiter.row.reclaimable());
    }
}
