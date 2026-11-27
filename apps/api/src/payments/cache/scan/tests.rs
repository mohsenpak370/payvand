use super::*;

/// The scan's window in the map: shared inside it, dropped at its end, and
/// never shared across a cluster or an endpoint.
#[test]
fn a_scan_is_shared_until_its_window_ends() {
    let mut inner = HashMap::new();
    let accounts = vec![(Pubkey::from([0x41u8; 32]), vec![1u8, 2, 3])];
    let start = Instant::now();
    assert!(
        cached_scan_in(&mut inner, "https://scan-test.invalid", "devnet", start).is_none(),
        "nothing has been scanned yet"
    );
    remember_scan_in(
        &mut inner,
        "https://scan-test.invalid",
        "devnet",
        &accounts,
        start,
    );
    let shared = cached_scan_in(
        &mut inner,
        "https://scan-test.invalid",
        "devnet",
        start + Duration::from_secs(3),
    )
    .expect("inside the window");
    assert_eq!(shared.len(), 1);
    assert!(
        cached_scan_in(
            &mut inner,
            "https://scan-test.invalid",
            "devnet",
            start + SCAN_TTL
        )
        .is_none(),
        "an expired scan is a miss"
    );
    assert!(
        inner.is_empty(),
        "the expired entry is dropped where it is found"
    );
}

#[test]
fn scans_are_kept_per_cluster_and_endpoint() {
    let mut inner = HashMap::new();
    let accounts = vec![(Pubkey::from([0x42u8; 32]), vec![1u8])];
    let start = Instant::now();
    remember_scan_in(
        &mut inner,
        "https://scan-a.invalid",
        "devnet",
        &accounts,
        start,
    );
    assert!(
        cached_scan_in(&mut inner, "https://scan-a.invalid", "mainnet-beta", start).is_none(),
        "another cluster is another program state"
    );
    assert!(
        cached_scan_in(&mut inner, "https://scan-b.invalid", "devnet", start).is_none(),
        "another endpoint has not been asked yet"
    );
    assert_eq!(inner.len(), 1);
}

#[test]
fn the_scan_cache_stays_bounded() {
    let mut inner = HashMap::new();
    let accounts = vec![(Pubkey::from([0x43u8; 32]), vec![1u8])];
    let start = Instant::now();
    for index in 0..(SCAN_CACHE_LIMIT + 3) {
        remember_scan_in(
            &mut inner,
            &format!("https://scan-{index}.invalid"),
            "devnet",
            &accounts,
            start + Duration::from_millis(index as u64),
        );
    }
    assert_eq!(inner.len(), SCAN_CACHE_LIMIT);
    assert!(
        cached_scan_in(&mut inner, "https://scan-0.invalid", "devnet", start).is_none(),
        "the oldest fell out"
    );
}
