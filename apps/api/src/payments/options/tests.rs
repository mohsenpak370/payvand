use crate::payments::test_support::sample_request;
use crate::payments::{create_payment, CreateOption, CreatePaymentRequest};

#[test]
fn prices_every_currency_the_seller_accepts() {
    let request = CreatePaymentRequest {
        amount_lamports: 0,
        options: vec![
            CreateOption {
                currency: "sol".into(),
                amount: 50_000_000,
            },
            CreateOption {
                currency: "usdc".into(),
                amount: 5_000_000,
            },
            CreateOption {
                currency: "eurc".into(),
                amount: 4_500_000,
            },
        ],
        ..sample_request()
    };
    let record = create_payment(request).expect("valid request");

    assert_eq!(record.options.len(), 3);
    assert_eq!(record.amount_lamports, 50_000_000);
    // SOL leads the payload: that is the shape older readers understand.
    assert_eq!(record.options[0].currency, "sol");
    assert_eq!(record.options[0].payment_pda, record.id);
    // Each currency gets its own address, so paying one cannot settle
    // another.
    let mut addresses = record
        .options
        .iter()
        .map(|option| option.payment_pda.clone())
        .collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();
    assert_eq!(addresses.len(), 3);

    let payload: serde_json::Value =
        serde_json::from_str(&record.qr_payload()).expect("payload is json");
    assert_eq!(payload["options"].as_array().unwrap().len(), 3);
    assert_eq!(payload["options"][1]["currency"], "usdc");
    assert_eq!(payload["options"][1]["decimals"], 6);
    assert_eq!(
        payload["options"][1]["mint"],
        "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
    );
}

#[test]
fn rejects_a_currency_twice_and_an_empty_invoice() {
    let twice = CreatePaymentRequest {
        amount_lamports: 0,
        options: vec![
            CreateOption {
                currency: "usdc".into(),
                amount: 1,
            },
            CreateOption {
                currency: "USDC".into(),
                amount: 2,
            },
        ],
        ..sample_request()
    };
    assert!(create_payment(twice).is_err());

    let nothing = CreatePaymentRequest {
        amount_lamports: 0,
        options: vec![],
        ..sample_request()
    };
    assert!(create_payment(nothing).is_err());
}

#[test]
fn rejects_a_token_that_has_no_mint_on_this_cluster() {
    let request = CreatePaymentRequest {
        amount_lamports: 0,
        options: vec![CreateOption {
            currency: "dai".into(),
            amount: 1_000_000,
        }],
        ..sample_request()
    };
    assert!(create_payment(request).is_err());
}
