//! Borsh reader and Anchor discriminators for program-owned accounts.

use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

/// `sha256("account:<Name>")[..8]`, the discriminator Anchor prefixes to
/// every `#[account]`.
pub(crate) fn account_discriminator() -> [u8; 8] {
    discriminator_for("Payment")
}

/// Discriminator of any Anchor account, for telling the program's account
/// kinds apart in a scan.
pub(crate) fn discriminator_for(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("account:{name}").as_bytes());
    digest[..8].try_into().expect("8 byte slice")
}

/// `sha256("event:<Name>")[..8]`, the prefix Anchor puts on an event's
/// `Program data:` line.
///
/// Events and accounts of the same name hash differently, and an event read
/// with the account prefix never matches: every settlement would decode to
/// `None`, which left settled invoices with no payer, price or seller — so a
/// fresh payment vanished from the history that had to name its owner.
pub(crate) fn event_discriminator_for(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("event:{name}").as_bytes());
    digest[..8].try_into().expect("8 byte slice")
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    pub(crate) fn pubkey(&mut self) -> Option<Pubkey> {
        let bytes: [u8; 32] = self.take(32)?.try_into().ok()?;
        Some(Pubkey::from(bytes))
    }

    pub(crate) fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    pub(crate) fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    pub(crate) fn string(&mut self) -> Option<String> {
        let len = usize::try_from(self.u32()?).ok()?;
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }
}
