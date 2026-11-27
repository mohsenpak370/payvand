use super::*;

#[test]
fn discriminator_matches_the_idl() {
    // `settle_payment` in apps/web/app/assets/payvand.idl.json.
    assert_eq!(
        settle_payment_discriminator(),
        [129, 7, 163, 250, 122, 226, 158, 249]
    );
}

#[test]
fn builds_a_legacy_transaction_a_wallet_can_sign() {
    let payer = Pubkey::new_from_array([1u8; 32]);
    let payment = Pubkey::new_from_array([9u8; 32]);
    let seller = Pubkey::new_from_array([2u8; 32]);
    let reference = Pubkey::new_from_array([3u8; 32]);
    let blockhash = [7u8; 32];

    let wire = settle_payment_transaction(SettlePaymentRequest {
        payer,
        payment,
        seller,
        reference,
        amount: 10_000_000,
        memo: "Coffee",
        blockhash,
    });

    // Unsigned: no signature slots, just the message the wallet signs.
    assert_eq!(wire[0], 0);

    let message = &wire[1..];
    let mut offset = 0;
    assert_eq!(&message[offset..offset + 3], &[1, 0, 3]);
    offset += 3;

    assert_eq!(message[offset], 6);
    offset += 1;
    let keys = &message[offset..offset + 6 * 32];
    for (index, key) in [payer, payment, seller, reference].iter().enumerate() {
        assert_eq!(&keys[index * 32..(index + 1) * 32], key.as_ref());
    }
    offset += 6 * 32;

    assert_eq!(&message[offset..offset + 32], &blockhash);
    offset += 32;

    assert_eq!(message[offset], 1); // one instruction
    offset += 1;
    assert_eq!(message[offset], 5); // program id index
    offset += 1;
    assert_eq!(message[offset], 5); // account count
    offset += 1;
    assert_eq!(&message[offset..offset + 5], &[0, 1, 2, 3, 4]);
    offset += 5;

    let data_len = message[offset] as usize;
    offset += 1;
    let data = &message[offset..offset + data_len];
    assert_eq!(&data[..8], &settle_payment_discriminator());
    assert_eq!(&data[8..16], &10_000_000u64.to_le_bytes());
    assert_eq!(&data[16..20], &6u32.to_le_bytes());
    assert_eq!(&data[20..], b"Coffee");
    assert_eq!(offset + data_len, message.len());
}
