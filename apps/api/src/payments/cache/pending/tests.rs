use super::*;
use crate::payments::onchain::records::pending_record;
use crate::payments::PaymentStatus;

/// An unsettled answer is shared for one window and then asked for again.
/// The length is the point: long enough that the buyer's page, a second tab
/// and the seller's screen share one cluster read, short enough that a
/// payment still appears within a poll or two.
#[test]
fn an_unsettled_answer_is_shared_until_its_window_ends() {
    let endpoint = "https://pending-test.invalid";
    let invoice = Pubkey::from([7u8; 32]);
    let record = pending_record(invoice, "devnet".into(), endpoint.into());
    let mut inner = PendingCacheInner::default();
    let start = Instant::now();

    assert!(
        cached_pending_in(&mut inner, endpoint, &invoice, start).is_none(),
        "nothing has been read yet"
    );

    remember_pending_in(&mut inner, endpoint, invoice, &record, start);
    let shared = cached_pending_in(
        &mut inner,
        endpoint,
        &invoice,
        start + Duration::from_secs(3),
    )
    .expect("a second viewer reads the first one's answer");
    assert_eq!(shared.id, invoice.to_string());
    assert!(
        cached_pending_in(&mut inner, endpoint, &invoice, start + PENDING_TTL).is_none(),
        "an expired answer is a miss, not a stale status"
    );
    assert!(
        inner.entries.is_empty(),
        "the expired entry is dropped where it is found"
    );
}

/// A settled record belongs to the cache that never expires; a copy with a
/// deadline here would only make the next read slower.
#[test]
fn a_settled_record_is_not_kept_as_pending() {
    let endpoint = "https://pending-test-settled.invalid";
    let invoice = Pubkey::from([8u8; 32]);
    let mut record = pending_record(invoice, "devnet".into(), endpoint.into());
    record.status = PaymentStatus::Settled;
    let mut inner = PendingCacheInner::default();
    remember_pending_in(&mut inner, endpoint, invoice, &record, Instant::now());
    assert!(inner.entries.is_empty());
}

/// A cluster keeps its own answers: two networks can hold different states
/// at the same address, so one must never answer for the other.
#[test]
fn the_pending_memo_is_keyed_by_endpoint() {
    let endpoint = "https://pending-test-a.invalid";
    let other = "https://pending-test-b.invalid";
    let invoice = Pubkey::from([9u8; 32]);
    let record = pending_record(invoice, "devnet".into(), endpoint.into());
    let mut inner = PendingCacheInner::default();
    let start = Instant::now();
    remember_pending_in(&mut inner, endpoint, invoice, &record, start);
    assert!(cached_pending_in(&mut inner, endpoint, &invoice, start).is_some());
    assert!(
        cached_pending_in(&mut inner, other, &invoice, start).is_none(),
        "another endpoint has not been asked yet"
    );
}

/// A `null` answer is a status — "not paid yet" — and is worth keeping for
/// the window like any other unsettled one.
#[test]
fn an_unpaid_answer_is_kept_for_its_window() {
    let endpoint = "https://pending-test-null.invalid";
    let invoice = Pubkey::from([0xAu8; 32]);
    let record = pending_record(invoice, "devnet".into(), endpoint.into());
    assert_eq!(record.status, PaymentStatus::Pending);
    let mut inner = PendingCacheInner::default();
    let start = Instant::now();
    remember_pending_in(&mut inner, endpoint, invoice, &record, start);
    assert!(cached_pending_in(&mut inner, endpoint, &invoice, start).is_some());
}
