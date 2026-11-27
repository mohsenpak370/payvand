use super::*;
use crate::payments::onchain::accounts::settled_event_in_log;
use crate::payments::test_support::settled_log_line;

/// A bounded cache, so a long-running instance cannot grow one entry per
/// settlement ever made.
#[test]
fn the_settled_cache_evicts_its_oldest_entry() {
    let endpoint = "https://cache-test-bound.invalid";
    let mut inner = SettledCacheInner {
        entries: HashMap::new(),
        order: VecDeque::new(),
    };

    let mut invoices = Vec::new();
    for index in 0..(SETTLED_CACHE_LIMIT + 8) {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
        let invoice = Pubkey::from(bytes);
        let line = settled_log_line(invoice, Pubkey::default(), 1_000, "x");
        remember_in(
            &mut inner,
            endpoint,
            invoice,
            &settled_event_in_log(&line).expect("decodes"),
        );
        invoices.push(invoice);
    }

    let newest = *invoices.last().expect("inserted at least one");
    assert!(inner.entries.contains_key(&(endpoint.to_string(), newest)));
    assert!(
        !inner
            .entries
            .contains_key(&(endpoint.to_string(), invoices[0])),
        "the oldest settlement should have been evicted"
    );
    assert_eq!(inner.entries.len(), SETTLED_CACHE_LIMIT);
}
