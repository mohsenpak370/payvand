# SPL program fixtures

The token paths in `../settle.rs` pay real token invoices, which means the
program under test CPIs into the SPL token program and the associated-token
program. `solana-program-test` ships neither, and `ProgramTest::add_program` only
finds ELFs that our own build wrote to `SBF_OUT_DIR`, so both are committed here
and registered by `program_test()` as executable accounts — exactly what
`add_program` would do with an artifact it found.

Without them the harness could only write token accounts and budgets by hand and
never execute a transfer, a vault creation, or a close.

## Provenance

Both files are the deployed programs, dumped from **devnet on 2026-11-15** with:

```bash
solana program dump TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA \
  programs/payvand/tests/fixtures/spl_token.so --url devnet
solana program dump ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL \
  programs/payvand/tests/fixtures/spl_associated_token_account.so --url devnet
```

| File | Program | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `spl_token.so` | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` | 108,600 | `8190d3f7ceb6cb7a7a8d8924bff89f9f611e15ce1f806f2b6237f3311a98f697` |
| `spl_associated_token_account.so` | `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL` | 105,032 | `6804554e69fd3a58caa191dc4a58f4c67223d30ca28ab8987f39fc18d2f7374d` |

Both dumps are the legacy SBF target (`e_machine` `0x0107`) and load in this
runtime; the associated-token program's deployment is the eBPF build
(`0x00f7`), but it executes here as well, which is why it is registered rather
than stubbed.

Licensing: both are Apache-2.0 (Solana Program Library). They are binary
third-party artifacts kept only as test fixtures; refresh them from the program
IDs above when a test needs behaviour the dumps predate.
