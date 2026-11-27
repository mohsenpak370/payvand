use super::*;
use solana_sdk::pubkey::Pubkey;

#[test]
fn a_mint_outside_the_list_is_refused() {
    let mut request = SettleRequest {
        transaction: String::new(),
        buyer: Pubkey::new_from_array([1; 32]).to_string(),
        seller: Pubkey::new_from_array([2; 32]).to_string(),
        reference: Pubkey::new_from_array([3; 32]).to_string(),
        mint: Pubkey::new_from_array([7; 32]).to_string(),
        amount: 1_000_000,
        memo: String::new(),
        shop_policy: None,
        cluster: "devnet".into(),
        endpoint: None,
    };
    assert!(Params::read(&request).is_err());
    request.mint = crate::payments::mint_for("usdc", "devnet")
        .expect("USDC")
        .to_string();
    assert!(Params::read(&request).is_ok());
    request.shop_policy = Some("not-a-key".into());
    assert!(Params::read(&request).is_err());
    request.shop_policy = Some(String::new());
    assert_eq!(
        Params::read(&request)
            .expect("blank means no policy")
            .shop_policy,
        None
    );
    request.amount = 0;
    assert!(Params::read(&request).is_err());
    request.amount = 1;
    request.memo = "x".repeat(97);
    assert!(Params::read(&request).is_err());
}
