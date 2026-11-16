//! Live list updates (MB-0057, RR-0049): the hub the routes announce
//! changes to, and the event stream each open list page listens on.
//!
//! - **No content.** A stream writes only the event `changed`, the event
//!   `stop` and keep-alive comments (RR-0049 constraint 1). On `changed` the
//!   page re-fetches its own fragment through `GET /lists/{id}` or
//!   `GET /lists/{id}/shop`, which check access, language and theme as for
//!   any request.
//! - **Re-checked.** Before every `changed` the central access predicate
//!   runs again for that viewer, through `ShoppingListRepo::access`; no
//!   grant is kept between events (constraint 2, RR-0002 constraint 4). When
//!   it fails the viewer gets one last `changed`, then `stop`, and the
//!   stream ends.
//! - **Bounded.** [`STREAMS_PER_USER`], [`STREAMS_TOTAL`], [`KEEP_ALIVE`]
//!   and [`STREAM_MAX_AGE`] change only through a consult (constraint 5).
//! - **The lock** is a `std::sync::Mutex`, never held across `.await`; a
//!   poisoned lock is recovered.
//!
//! This file is a trigger path ("Shared-list authorization", `feature.md`).

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;

use actix_web::body::{BodySize, MessageBody};
use actix_web::web::Bytes;
use tokio::sync::{Notify, mpsc};
use tokio::time::Instant;
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::repo::{ShoppingListRepo, SqlitePool};

/// Open streams one user may hold (RR-0049 sub-answer 5).
pub const STREAMS_PER_USER: usize = 8;
/// Open streams in total (RR-0049 sub-answer 5).
pub const STREAMS_TOTAL: usize = 200;
/// A comment line is written after this long without an event, so proxies
/// keep the connection and a gone client is noticed.
pub const KEEP_ALIVE: Duration = Duration::from_secs(20);
/// Every stream ends after this long, without `stop`, so the reconnect
/// passes the identity middleware again (sign-out, expiry).
pub const STREAM_MAX_AGE: Duration = Duration::from_secs(15 * 60);

const CHANGED: &[u8] = b"event: changed\ndata:\n\n";
const STOP: &[u8] = b"event: stop\ndata:\n\n";
const KEEP_ALIVE_COMMENT: &[u8] = b": keep-alive\n\n";

/// The hub's limits. `main` and the test app use [`Limits::default`]; tests
/// of the limits pass smaller values and short durations.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub per_user: usize,
    pub total: usize,
    pub keep_alive: Duration,
    pub max_age: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            per_user: STREAMS_PER_USER,
            total: STREAMS_TOTAL,
            keep_alive: KEEP_ALIVE,
            max_age: STREAM_MAX_AGE,
        }
    }
}

/// Why a connect is refused with 503.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    PerUser,
    Total,
    ShuttingDown,
}

impl Refused {
    /// The limit's name for the WARN line.
    pub const fn limit(self) -> &'static str {
        match self {
            Self::PerUser => "streams_per_user",
            Self::Total => "streams_total",
            Self::ShuttingDown => "shutting_down",
        }
    }
}

/// Who made a change: a write that carries its page's tab id is not
/// announced to that one page (RR-0049 sub-answer 8). The id authorizes
/// nothing.
#[derive(Debug, Clone, Copy)]
pub struct Writer {
    pub user: Uuid,
    pub tab: Uuid,
}

/// What one stream waits on. Signals coalesce: several changes before the
/// stream wakes are one `changed`.
#[derive(Debug, Default)]
struct Signal {
    changed: AtomicBool,
    closed: AtomicBool,
    notify: Notify,
}

impl Signal {
    fn change(&self) {
        self.changed.store(true, Ordering::Release);
        self.notify.notify_one();
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.notify.notify_one();
    }
}

/// One open stream: the viewer, the list's owner, the page's tab id, and
/// the signal. Never list content, never a grant.
#[derive(Debug)]
struct Subscriber {
    id: u64,
    user: Uuid,
    owner: Uuid,
    tab: Option<Uuid>,
    signal: Arc<Signal>,
}

#[derive(Debug, Default)]
struct Inner {
    /// Counts every announcement; a page render records it ([`LiveHub::mark`]).
    seq: u64,
    next_id: u64,
    closed: bool,
    total: usize,
    per_user: HashMap<Uuid, usize>,
    subscribers: HashMap<Uuid, Vec<Subscriber>>,
    /// The `seq` of each list's last announcement, for catch-up.
    list_seq: HashMap<Uuid, u64>,
    /// The same for [`LiveHub::owner_changed`].
    owner_seq: HashMap<Uuid, u64>,
}

/// The in-process change notification (RR-0049 sub-answer 4): one per
/// process, in `AppState`.
pub struct LiveHub {
    limits: Limits,
    /// Random per process, so a mark from before a restart is unknown.
    epoch: Uuid,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for LiveHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveHub")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl LiveHub {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            epoch: Uuid::new_v4(),
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The point a page render starts from, carried in its stream URL
    /// (`since`). Take it before reading the list, so a change during the
    /// render is caught up.
    pub fn mark(&self) -> String {
        let seq = self.lock().seq;
        format!("{}.{seq}", self.epoch.simple())
    }

    /// `since` as a `seq` of this process, or `None` when the server can't
    /// tell (absent, malformed, or from before a restart).
    fn parse_mark(&self, since: Option<&str>) -> Option<u64> {
        let (epoch, seq) = since?.split_once('.')?;
        if epoch != self.epoch.simple().to_string() {
            return None;
        }
        seq.parse().ok()
    }

    /// Announce a change to what `list` shows, after the write succeeded.
    /// Every stream on the list is signalled except the writer's own page.
    pub fn changed(&self, list: Uuid, writer: Option<Writer>) {
        let mut inner = self.lock();
        inner.seq += 1;
        let seq = inner.seq;
        inner.list_seq.insert(list, seq);
        for sub in inner.subscribers.get(&list).into_iter().flatten() {
            let own = writer.is_some_and(|w| w.user == sub.user && Some(w.tab) == sub.tab);
            if !own {
                sub.signal.change();
            }
        }
    }

    /// Announce every list `owner` owns: their categories changed, and
    /// lists show the categories' live names and order (RR-0049 sub-answer
    /// 6).
    pub fn owner_changed(&self, owner: Uuid) {
        let mut inner = self.lock();
        inner.seq += 1;
        let seq = inner.seq;
        inner.owner_seq.insert(owner, seq);
        for sub in inner.subscribers.values().flatten() {
            if sub.owner == owner {
                sub.signal.change();
            }
        }
    }

    /// Close every stream, without `stop`, and refuse new ones: the server
    /// is shutting down (RR-0049 sub-answer 5).
    pub fn close_all(&self) {
        let mut inner = self.lock();
        inner.closed = true;
        for sub in inner.subscribers.values().flatten() {
            sub.signal.close();
        }
    }

    /// Take a stream slot for `user`, before the access predicate runs.
    /// Open streams are never evicted.
    pub fn reserve(self: &Arc<Self>, user: Uuid) -> Result<Slot, Refused> {
        let mut inner = self.lock();
        if inner.closed {
            return Err(Refused::ShuttingDown);
        }
        if inner.total >= self.limits.total {
            return Err(Refused::Total);
        }
        let mine = inner.per_user.entry(user).or_insert(0);
        if *mine >= self.limits.per_user {
            return Err(Refused::PerUser);
        }
        *mine += 1;
        inner.total += 1;
        inner.next_id += 1;
        Ok(Slot {
            hub: Arc::clone(self),
            id: inner.next_id,
            user,
            list: None,
            signal: Arc::new(Signal::default()),
        })
    }

    fn release(&self, slot: &Slot) {
        let mut inner = self.lock();
        inner.total = inner.total.saturating_sub(1);
        if let Some(mine) = inner.per_user.get_mut(&slot.user) {
            *mine = mine.saturating_sub(1);
            if *mine == 0 {
                inner.per_user.remove(&slot.user);
            }
        }
        if let Some(list) = slot.list
            && let Some(subs) = inner.subscribers.get_mut(&list)
        {
            subs.retain(|s| s.id != slot.id);
            if subs.is_empty() {
                inner.subscribers.remove(&list);
            }
        }
    }
}

/// A reserved stream slot; dropping it frees the slot and the
/// subscription.
#[derive(Debug)]
pub struct Slot {
    hub: Arc<LiveHub>,
    id: u64,
    user: Uuid,
    list: Option<Uuid>,
    signal: Arc<Signal>,
}

impl Slot {
    /// Subscribe to `list`, after the predicate passed for this user.
    /// Returns whether the stream starts with a catch-up `changed`: the
    /// list changed after `since`, or the server can't tell.
    fn subscribe(
        &mut self,
        list: Uuid,
        owner: Uuid,
        tab: Option<Uuid>,
        since: Option<&str>,
    ) -> bool {
        let since = self.hub.parse_mark(since);
        let mut inner = self.hub.lock();
        self.list = Some(list);
        inner.subscribers.entry(list).or_default().push(Subscriber {
            id: self.id,
            user: self.user,
            owner,
            tab,
            signal: Arc::clone(&self.signal),
        });
        if inner.closed {
            self.signal.close();
        }
        let catch_up = since.is_none_or(|since| {
            let after = |seq: Option<&u64>| seq.is_some_and(|seq| *seq > since);
            after(inner.list_seq.get(&list)) || after(inner.owner_seq.get(&owner))
        });
        drop(inner);
        catch_up
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.hub.release(self);
    }
}

/// An event stream's body: a hand-written `MessageBody` (no new
/// dependency, RR-0049 sub-answer 1) over the channel its task writes to.
/// Dropping it (the client went away) ends the task.
#[derive(Debug)]
pub struct LiveBody {
    rx: mpsc::Receiver<Bytes>,
}

impl MessageBody for LiveBody {
    type Error = std::convert::Infallible;

    fn size(&self) -> BodySize {
        BodySize::Stream
    }

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Bytes, Self::Error>>> {
        self.get_mut().rx.poll_recv(cx).map(|chunk| chunk.map(Ok))
    }
}

/// Open the stream for `slot`'s user on `list`. The caller has just passed
/// the access predicate for them and read the list's owner.
pub fn stream(
    mut slot: Slot,
    pool: SqlitePool,
    list: Uuid,
    owner: Uuid,
    tab: Option<Uuid>,
    since: Option<&str>,
) -> LiveBody {
    let catch_up = slot.subscribe(list, owner, tab, since);
    let (tx, rx) = mpsc::channel(4);
    let user = slot.user;
    tracing::debug!(user_id = %user, list_id = %list, "live stream opened");
    tokio::spawn(async move {
        let reason = run(&slot, &pool, list, &tx, catch_up).await;
        if reason == End::AccessEnded {
            tracing::info!(user_id = %user, list_id = %list, "live stream stopped: access ended");
        }
        tracing::debug!(user_id = %user, list_id = %list, reason = reason.as_str(), "live stream closed");
    });
    LiveBody { rx }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    ClientGone,
    MaxAge,
    Shutdown,
    AccessEnded,
    CheckFailed,
}

impl End {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ClientGone => "client_gone",
            Self::MaxAge => "max_age",
            Self::Shutdown => "shutdown",
            Self::AccessEnded => "access_ended",
            Self::CheckFailed => "check_failed",
        }
    }
}

async fn send(tx: &mpsc::Sender<Bytes>, chunk: &'static [u8]) -> Result<(), End> {
    tx.send(Bytes::from_static(chunk))
        .await
        .map_err(|_| End::ClientGone)
}

/// One stream's life, until it ends for the returned reason.
async fn run(
    slot: &Slot,
    pool: &SqlitePool,
    list: Uuid,
    tx: &mpsc::Sender<Bytes>,
    catch_up: bool,
) -> End {
    let limits = slot.hub.limits;
    let deadline = Instant::now() + limits.max_age;
    // The route passed the predicate a moment ago, so the catch-up signal
    // needs no second check.
    if catch_up && let Err(end) = send(tx, CHANGED).await {
        return end;
    }
    loop {
        tokio::select! {
            () = tx.closed() => return End::ClientGone,
            () = tokio::time::sleep_until(deadline) => return End::MaxAge,
            () = slot.signal.notify.notified() => {
                if slot.signal.closed.load(Ordering::Acquire) {
                    return End::Shutdown;
                }
                if slot.signal.changed.swap(false, Ordering::AcqRel)
                    && let Err(end) = announce(slot, pool, list, tx).await
                {
                    return end;
                }
            }
            () = tokio::time::sleep(limits.keep_alive) => {
                if let Err(end) = send(tx, KEEP_ALIVE_COMMENT).await {
                    return end;
                }
            }
        }
    }
}

/// Write one `changed`, after the predicate passed again for this viewer.
/// If their access ended: a last `changed` (their re-fetch answers the
/// error page), then `stop`.
async fn announce(
    slot: &Slot,
    pool: &SqlitePool,
    list: Uuid,
    tx: &mpsc::Sender<Bytes>,
) -> Result<(), End> {
    match ShoppingListRepo::access(pool, slot.user, list).await {
        Ok(_) => send(tx, CHANGED).await,
        Err(CoreError::NotFound | CoreError::Forbidden | CoreError::Unauthorized) => {
            send(tx, CHANGED).await?;
            send(tx, STOP).await?;
            Err(End::AccessEnded)
        }
        Err(error) => {
            // Not an answer about access: end the stream without `stop`, so
            // the page reconnects and the connect checks again.
            tracing::error!(user_id = %slot.user, list_id = %list, %error, "live stream re-check failed");
            Err(End::CheckFailed)
        }
    }
}
