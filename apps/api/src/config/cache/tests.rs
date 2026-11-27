use super::*;
use crate::config::test_support::sample_config;

/// One read answers every request for a minute, per cluster, and the entry is
/// dropped when it is looked at after that.
#[test]
fn a_config_is_kept_for_a_minute_per_cluster() {
    let devnet = sample_config();
    let mut inner = HashMap::new();
    let start = Instant::now();

    assert!(cached_config_in(&mut inner, "devnet", start).is_none());
    remember_config_in(&mut inner, "devnet", &devnet, start);

    let shared = cached_config_in(&mut inner, "devnet", start + Duration::from_secs(59))
        .expect("inside the minute");
    assert_eq!(shared.fees.fee_bps_below, 200);
    assert!(
        cached_config_in(&mut inner, "mainnet-beta", start).is_none(),
        "another cluster is another policy"
    );
    assert!(
        cached_config_in(&mut inner, "devnet", start + CONFIG_TTL).is_none(),
        "an expired answer is a miss"
    );
    assert!(
        inner.is_empty(),
        "the expired entry is dropped where it is found"
    );
}
