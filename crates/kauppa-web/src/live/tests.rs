//! The hub on its own (MB-0057, RR-0049 sub-answers 4 and 5). The streams
//! and the route are tested in `routes::lists::tests::live`.
#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::Ordering;

use uuid::Uuid;

use super::{Limits, LiveHub, Refused, Slot, Writer};

fn hub(per_user: usize, total: usize) -> Arc<LiveHub> {
    Arc::new(LiveHub::new(Limits {
        per_user,
        total,
        ..Limits::default()
    }))
}

/// A subscribed slot for `user` on `list`, whose mark is current.
fn subscribe(hub: &Arc<LiveHub>, user: Uuid, list: Uuid, owner: Uuid, tab: Option<Uuid>) -> Slot {
    let mut slot = hub.reserve(user).expect("a free slot");
    let since = hub.mark();
    assert!(!slot.subscribe(list, owner, tab, Some(&since)));
    slot
}

/// Whether `slot` was signalled since the last call.
fn signalled(slot: &Slot) -> bool {
    slot.signal.changed.swap(false, Ordering::AcqRel)
}

/// The constants are RR-0049 sub-answer 5's; changing one needs a consult
/// (constraint 5).
#[test]
fn constants_are_the_approved_ones() {
    use std::time::Duration;

    assert_eq!(super::STREAMS_PER_USER, 8);
    assert_eq!(super::STREAMS_TOTAL, 200);
    assert_eq!(super::KEEP_ALIVE, Duration::from_secs(20));
    assert_eq!(super::STREAM_MAX_AGE, Duration::from_secs(15 * 60));
    let limits = Limits::default();
    assert_eq!(
        (
            limits.per_user,
            limits.total,
            limits.keep_alive,
            limits.max_age
        ),
        (8, 200, super::KEEP_ALIVE, super::STREAM_MAX_AGE)
    );
}

/// The only bytes a stream writes (constraint 1): two bare events and a
/// comment.
#[test]
fn events_carry_nothing() {
    assert_eq!(super::CHANGED, b"event: changed\ndata:\n\n");
    assert_eq!(super::STOP, b"event: stop\ndata:\n\n");
    assert_eq!(super::KEEP_ALIVE_COMMENT, b": keep-alive\n\n");
}

/// Slots are bounded per user and in total; a refused connect takes
/// nothing, an open stream is never evicted, and a dropped slot is free
/// again.
#[test]
fn slots_are_bounded_and_freed() {
    let hub = hub(2, 3);
    let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let a1 = hub.reserve(a).expect("first");
    let _a2 = hub.reserve(a).expect("second");
    assert_eq!(hub.reserve(a).err(), Some(Refused::PerUser));
    let _b1 = hub.reserve(b).expect("another user");
    assert_eq!(hub.reserve(c).err(), Some(Refused::Total));
    // The total is checked first: at both caps the answer names it.
    assert_eq!(hub.reserve(a).err(), Some(Refused::Total));
    assert_eq!(hub.open_streams(), 3);

    drop(a1);
    assert_eq!(hub.open_streams(), 2);
    let _c1 = hub.reserve(c).expect("the freed slot");
    assert_eq!(hub.reserve(a).err(), Some(Refused::Total));
}

/// A change signals every stream on that list except the writer's own
/// page: same user and same tab. Signals coalesce.
#[test]
fn changed_signals_the_lists_streams() {
    let hub = hub(8, 200);
    let (owner, friend) = (Uuid::new_v4(), Uuid::new_v4());
    let (list, other_list) = (Uuid::new_v4(), Uuid::new_v4());
    let (tab, other_tab) = (Uuid::new_v4(), Uuid::new_v4());
    let writing = subscribe(&hub, owner, list, owner, Some(tab));
    let second = subscribe(&hub, owner, list, owner, Some(other_tab));
    let untabbed = subscribe(&hub, owner, list, owner, None);
    // Another user whose page happens to carry the same tab id.
    let same_tab = subscribe(&hub, friend, list, owner, Some(tab));
    let elsewhere = subscribe(&hub, owner, other_list, owner, Some(tab));

    let writer = Some(Writer { user: owner, tab });
    hub.changed(list, writer);
    hub.changed(list, writer);
    assert!(!signalled(&writing), "the writer's page is skipped");
    assert!(signalled(&second) && signalled(&untabbed) && signalled(&same_tab));
    assert!(!signalled(&second), "two changes were one signal");
    assert!(!signalled(&elsewhere), "another list");

    hub.changed(list, None);
    assert!(signalled(&writing), "a write without a tab id reaches all");
}

/// `owner_changed` signals the streams on that owner's lists, whoever is
/// viewing, and no one else's.
#[test]
fn owner_changed_signals_their_lists() {
    let hub = hub(8, 200);
    let (owner, friend, stranger) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let own = subscribe(&hub, owner, Uuid::new_v4(), owner, None);
    let shared = subscribe(&hub, friend, Uuid::new_v4(), owner, None);
    let friends_own = subscribe(&hub, friend, Uuid::new_v4(), friend, None);
    let unrelated = subscribe(&hub, stranger, Uuid::new_v4(), stranger, None);

    hub.owner_changed(owner);
    assert!(signalled(&own) && signalled(&shared));
    assert!(!signalled(&friends_own) && !signalled(&unrelated));
}

/// Catch-up: a new stream starts with `changed` when the list, or its
/// owner's categories, changed after the page's mark, or when the server
/// can't tell.
#[test]
fn catch_up_from_the_mark() {
    let hub = hub(8, 200);
    let (owner, list) = (Uuid::new_v4(), Uuid::new_v4());
    let catch_up = |since: Option<&str>| {
        let mut slot = hub.reserve(owner).expect("slot");
        slot.subscribe(list, owner, None, since)
    };

    let before = hub.mark();
    assert!(!catch_up(Some(&before)), "nothing changed since");
    hub.changed(Uuid::new_v4(), None);
    assert!(!catch_up(Some(&before)), "another list changed");
    hub.changed(list, None);
    assert!(catch_up(Some(&before)), "the list changed after the mark");
    let after = hub.mark();
    assert!(!catch_up(Some(&after)));
    hub.owner_changed(owner);
    assert!(catch_up(Some(&after)), "the owner's categories changed");

    // The server can't tell: no mark, a malformed one, another process's.
    assert!(catch_up(None));
    assert!(catch_up(Some("")));
    assert!(catch_up(Some("nonsense")));
    let seq = after.split_once('.').expect("epoch.seq").1;
    assert!(catch_up(Some(&format!(
        "{}.{seq}",
        Uuid::new_v4().simple()
    ))));
    assert!(catch_up(Some(&format!(
        "{}.x",
        after.split_once('.').expect("epoch.seq").0
    ))));
}

/// Shutdown closes every stream and refuses new ones.
#[test]
fn close_all_closes_and_refuses() {
    let hub = hub(8, 200);
    let user = Uuid::new_v4();
    let slot = subscribe(&hub, user, Uuid::new_v4(), user, None);
    hub.close_all();
    assert!(slot.signal.closed.load(Ordering::Acquire));
    assert_eq!(hub.reserve(user).err(), Some(Refused::ShuttingDown));
}

/// A poisoned lock is recovered, not unwrapped (RR-0049 sub-answer 4).
#[test]
fn poisoned_lock_is_recovered() {
    let hub = hub(8, 200);
    let poisoner = Arc::clone(&hub);
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.inner.lock();
        std::panic::resume_unwind(Box::new("poison the hub's lock"));
    })
    .join();
    assert!(hub.inner.is_poisoned());
    hub.changed(Uuid::new_v4(), None);
    assert!(hub.reserve(Uuid::new_v4()).is_ok());
}

/// A connect, accepted or refused, makes that user's open streams write a
/// keep-alive comment at once, so the ones whose page is gone are found;
/// other users' streams aren't touched.
#[test]
fn a_connect_probes_the_users_streams() {
    let hub = hub(2, 200);
    let (user, other) = (Uuid::new_v4(), Uuid::new_v4());
    let list = Uuid::new_v4();
    let probed = |slot: &Slot| slot.signal.probe.swap(false, Ordering::AcqRel);
    let first = subscribe(&hub, user, list, user, None);
    let others = subscribe(&hub, other, list, user, None);
    assert!(!probed(&first));

    let second = subscribe(&hub, user, list, user, None);
    assert!(probed(&first) && !probed(&second) && !probed(&others));

    assert_eq!(hub.reserve(user).err(), Some(Refused::PerUser));
    assert!(probed(&first) && probed(&second) && !probed(&others));
}
