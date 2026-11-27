//! What a refusal logs, so an operator can tell a stale client, a wrong
//! invoice and a probing caller apart without the transaction itself.

use solana_sdk::message::VersionedMessage;

use super::params::Params;

/// A caller's word as a log line may keep it: trimmed and bounded, so a request
/// cannot choose the size of the log. Only fields that are public anyway are
/// logged — an address, a cluster — never the memo, which is the buyer's.
pub(super) fn logged(value: &str) -> String {
    value.trim().chars().take(64).collect()
}

/// The mint a refusal names, with `SOL` standing for the wallet path's absence
/// of one — an operator reading the log should not have to know that an empty
/// mint is the lamport invoice.
pub(super) fn mint_label(params: &Params) -> String {
    params
        .mint
        .map(|mint| mint.to_string())
        .unwrap_or_else(|| "SOL".into())
}

/// The shape of a settle message: its account count, the instruction's account
/// count, and the instruction data's length. A missing shop policy, a different
/// buyer token account or a different price each change one of the three, which
/// is what makes a refusal readable without the transaction itself.
pub(super) fn shape(message: &VersionedMessage) -> (usize, usize, usize) {
    match message {
        VersionedMessage::V0(message) => {
            let instruction = message.instructions.first();
            (
                message.account_keys.len(),
                instruction.map_or(0, |instruction| instruction.accounts.len()),
                instruction.map_or(0, |instruction| instruction.data.len()),
            )
        }
        _ => (0, 0, 0),
    }
}

/// Where two serialized settle messages first part ways. A version 0 message
/// ends with the blockhash, so an offset inside the keys or the instruction is
/// a stale client, a wrong invoice or a missing policy — and each of those is a
/// different bug for whoever reads the log.
pub(super) fn first_difference(ours: &[u8], theirs: &[u8]) -> Option<usize> {
    ours.iter()
        .zip(theirs.iter())
        .position(|(ours, theirs)| ours != theirs)
        .or_else(|| (ours.len() != theirs.len()).then_some(ours.len().min(theirs.len())))
}

#[cfg(test)]
mod tests;
