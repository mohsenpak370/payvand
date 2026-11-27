use super::*;

fn key(text: &str) -> Pubkey {
    Pubkey::from_str(text).expect("valid address")
}

#[test]
fn moves_the_named_lamports_to_the_linked_wallet() {
    let from = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
    let to = key("3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx");
    let instruction = sol_transfer_ix(&from, &to, 1_000_000_000);

    assert_eq!(instruction.program_id, key(SYSTEM_PROGRAM));
    assert_eq!(&instruction.data[..4], &2u32.to_le_bytes());
    assert_eq!(&instruction.data[4..], &1_000_000_000u64.to_le_bytes());
    assert_eq!(instruction.accounts[0], AccountMeta::new(from, true));
    assert_eq!(instruction.accounts[1], AccountMeta::new(to, false));
}

#[test]
fn transfers_the_named_amount_in_little_endian() {
    let source = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
    let destination = key("3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx");
    let instruction = token_transfer_ix(&source, &destination, &source, 10_000_000);

    assert_eq!(instruction.program_id, key(TOKEN_PROGRAM));
    assert_eq!(instruction.data[0], 3);
    assert_eq!(instruction.data.len(), 9);
    assert_eq!(&instruction.data[1..], &10_000_000u64.to_le_bytes());
    assert_eq!(
        instruction.accounts,
        vec![
            AccountMeta::new(source, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(source, true),
        ]
    );
}

#[test]
fn creates_the_token_account_only_when_it_is_missing() {
    let payer = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
    let mint = key("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
    let owner = key("3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx");
    let instruction = create_ata_ix(&payer, &owner, &mint);

    assert_eq!(instruction.program_id, key(ATA_PROGRAM));
    // `CreateIdempotent`, not `Create`: a wallet that already holds USDC must
    // not make the whole claim fail.
    assert_eq!(instruction.data, vec![1]);
    assert_eq!(instruction.accounts[1].pubkey, ata_for(&owner, &mint));
    assert!(instruction.accounts[0].is_signer);
    assert!(instruction.accounts[0].is_writable);
}

#[test]
fn derives_the_same_token_account_the_program_would() {
    // ATA = PDA(["<owner>", "<token program>", "<mint>"], ATA program); the
    // example is the treasury's devnet USDC account from the running app.
    let owner = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
    let mint = key("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
    let derived = ata_for(&owner, &mint);
    assert_eq!(derived, ata_for(&owner, &mint));
    assert_ne!(
        derived,
        ata_for(&owner, &key("HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr"))
    );
}
