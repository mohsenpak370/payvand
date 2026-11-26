# 0006 — Compressed payment receipts (4 sharded trees + rotation)

Status: `Deferred`. Not approved for implementation; moved from `plans/` to
`plans/deferred/` on 2026-11-26 because its trigger is traffic, not a date.

## When To Implement

Yes — **settlement volume is the trigger**. Do not build this on a schedule:

- **Start when the product expects ~700,000 settlements** on a cluster (the
  figure below is the point where owning paging, nullifier epochs, rotation and
  proof refresh pays for its surface area). Recheck the forecast and the
  observed settled-invoice count before starting any Phase 1 task.
- **Below that, keep the 48-byte marker.** Rent breaks even much earlier —
  roughly **7,700–8,000** invoices, against ~6.6 SOL of generation-0 shared
  accounts and ~894,080 devnet / 1,224,960 mainnet lamports saved per invoice —
  but rent is not the whole cost. Engineering, operations and review of the
  shared infrastructure dominate until the ~700k horizon.
- **Even at that volume, three gates must pass first:** Task 0.2's live devnet
  proof of the raw-CPI tree setup, Task 0.3's compatible compression program on
  the target cluster (mainnet's canonical program lacks prepared-init), and the
  nullifier-rule reconciliation noted below. The operator must also be ready to
  fund ~6.6 SOL of shared accounts plus 3.900 SOL per later generation.

## Progress Audit

Audited 2026-11-26 against the working tree at `92010cb`: **no implementation
task has started.** Phase 1–5 checkboxes remain open — there is no
`CompressionRegistry`, `ReceiptMode`, nullifier page, occupancy bitmap, tree CLI,
`replaceLeaf` CPI, `ReceiptCommitted` event, proof service, schema column, or
rotation instruction in the program, API, web app or IDL; replay is still the
48-byte marker only, and the live fee/refund math is still the legacy one. The
Task 0.3 mainnet compatibility gate has not been run (`DEV.md` records no
compression program IDs). What *is* done is the documentation half of Phase 0:
the fee decision and replay design are recorded below, and `README.md:454-475`
states the planned policy and labels it unshipped (Task 0.4). Before Task 2.1,
reconcile "both modes claim a nullifier" (above) with Task 2.1's "legacy path
need not require a nullifier page"; they cannot both describe a settle with no
page account.

## Replay Design Decision (review 2026-11-22)

The original draft trusted the indexer to probe for a free leaf, which was not an
on-chain replay guard: a client could submit the same invoice into another empty
slot or into a `previous` tree after rotation. This is resolved by separating
*invoice identity* from *tree slot allocation*:

- **Global tagged nullifier pages.** Each invoice hash selects one page within
  the active nullifier epoch and an 8-byte tag. Epoch 0 is 8 pages (65,536
  claims, 2.67 SOL) and grows 4× per epoch; the 683 SOL table is the lifetime
  ceiling, not the starting cost. The program probes that page on chain, stops
  if it finds the invoice's own tag (`AlreadySettled`), and claims the first
  empty tag slot. A plain bitmap was rejected because a replay could skip a set
  bit that belonged to another invoice; tagged slots identify the owner.
- **Per-generation shard occupancy bitmaps.** Tree-slot collisions are resolved
  by a separate bitmap per generation and shard. These are writable but
  partitioned, so four-way settlement parallelism is preserved. They reset on
  rotation; the global nullifier pages do not, so replay across rotations is
  blocked.
- **Fee rollback is preserved.** The compressed path ignores rent-derived flats,
  but the flat fields and price-refresh instructions stay active for the legacy
  marker path. The TS quote mirrors the active flag instead of hard-coding the
  compressed numbers before the flag is enabled.
- **The nullifier is canonical for every new settlement.** Both flag-off and
  flag-on settlements claim the same nullifier in the same atomic transaction as
  the transfer. Both paths also reject an existing legacy marker, so invoices
  settled before this program upgrade stay protected even though they have no
  nullifier. A flag flip cannot re-open an already-settled invoice. Before a
  legacy marker is ever reclaimed, its tag must be backfilled into the nullifier
  table.

## Compatibility Gate (review 2026-11-22)

The 36-byte program account bytes are identical across devnet and mainnet, but
that only encodes the same programdata address. The programdata accounts
themselves differ: devnet is 2,573,909 bytes (`4a454344…b685b9`), mainnet is
2,349,085 bytes (`2d104c51…d30c9d5`). The mainnet build exposes `replaceLeaf`
but not `prepare_batch_merkle_tree`, `append_canopy_nodes`, or
`init_prepared_tree_with_root`; those are marked "Devnet only" in current SPL.

Devnet implementation may proceed with the canonical devnet program, but mainnet
rollout requires a mainnet-compatible compression program. The chosen route is
to deploy or have upgraded a reviewed SPL-compatible build that exposes the
prepared-init instruction set, and record its cluster-specific program ID in
`DEV.md`. The canonical `cmtDvXum…` ID may be used on a cluster only after the
full prepare/canopy/init/replaceLeaf sequence is executed there successfully.

## Linked Docs

- `README.md` — "Fees, rent and the treasury" and "Notes and next steps"
- `DEV.md` — local cluster addresses and commands
- `plans/archive/0002-fee-model-and-treasury.md` — the fee/rent model this plan changes
- `plans/0004-relayer-sponsored-fees.md` — sponsored path also moves receipt rent
- `plans/0005-per-shop-fees.md` — fee overrides share `Config::policy`
- `apps/contracts/programs/payvand/src/state.rs` — the current `PaymentReceipt` marker

## What exists today

- Each settled invoice writes a permanent 48-byte `PaymentReceipt` at
  `["payment-receipt", invoice]`. It holds only `invoice` and `settled_at`;
  `settled_at != 0` is the atomic replay guard. There is no close instruction
  (`README.md:495`).
- Devnet's rent for 48 bytes is **894,080 lamports** — mainnet's 6,960 a byte
  makes it 1,224,960, the figure the program's constants quote, and every rent
  number in this plan is at devnet's 5,080 unless it says otherwise. Every
  settle funds it, and the fee policy is built around that deposit:
  - `fee_for` (SOL) subtracts `receipt_rent` from the proportional fee
    (`helpers.rs:84`).
  - Token flats are `flat_for_rent(receipt_rent, price)` and refreshed from the
    cluster rent (`config.rs:308`, `config.rs:361`).
  - Treasury refunds the receipt rent to the payer on the token path, and the
    seller absorbs it via the flat or the SOL deduction.
- The full receipt already lives off-chain: the `PaymentSettled` event in the
  settle transaction's logs, reconstructed by `settled_from_history`
  (`apps/api/src/payments.rs:1291`) with a 32-read history budget, an isolate
  cache, and the `settled_payments` libSQL table.

## Objective

Replace the per-invoice receipt account with a commitment in one of four shared
SPL concurrent Merkle trees, keeping an on-chain replay guard and adding tree
rotation so capacity is never a hard stop. At epoch-0 sizing the whole system
costs **~6.6 SOL of shared accounts and ~41,650 lamports per settled invoice**
(~42,580 once a generation's 3.900 SOL rotation rent is counted), against
894,080 lamports of permanent receipt rent today — about 21× cheaper
per invoice, with no per-invoice account at all. Existing receipt-history reads
and seller/payer/treasury accounting are preserved.

### Cost model (review 2026-11-22)

Rent is `(128 + bytes) × 5,080` lamports at devnet's per-byte rate, paid once
and permanently. The figures below are what the plan actually commits to; on
mainnet's 6,960 a byte each one scales by 1.37 (the 6.57 SOL becomes ~9.00 and
the legacy marker 1,224,960), so the ratios, the per-receipt share and the
break-even in invoices do not move.

| Item | Size | Rent, one-time | Per receipt at full |
| --- | --- | --- | --- |
| 4 trees (depth 20, buffer 64, canopy 8) | 4 × 60,600 B | 1.234 SOL | 294 |
| 4 occupancy bitmaps, generation 0 | 4 × 131,080 B | 2.666 SOL | 635 |
| Nullifier epoch 0, 8 pages | 8 × 65,544 B | 2.669 SOL | 40,725 |
| **Total** | | **~6.57 SOL** | **~41,650** |

That table is generation 0 and nullifier epoch 0 only. Rotation repeats the
tree and bitmap lines, leaves the nullifier pages alone, and locks another
permanent amount each time:

| Rotation step | Payer | Rent, every generation |
| --- | --- | --- |
| Step 1, four new trees (`tree rotate --prepare`, off chain) | operator keypair (`Config::admin`) | 1.234 SOL |
| Step 2, four new occupancy bitmaps (`rotate_generation`, on chain) | treasury PDA (`invoke_signed`) | 2.666 SOL |
| **Total per rotation** | | **3.900 SOL** |

- A rotation costs **3.900 SOL**, two thirds of it from the treasury and one
  third from the operator's own wallet. Generation 0's share is already in the
  6.57 SOL above, so enabling the flag does not cost more than that table says;
  what the table does not show is that every later generation adds another
  3.900 SOL that is never reclaimed.
- Spread over a full generation (4,194,304 receipts) that is **~930 lamports per
  receipt**, so steady state is ~42,580 lamports rather than ~41,650. Rotations
  are triggered at 60% shard fill, where the same 3.900 SOL works out to about
  **1,550 lamports per receipt**. Either way the compressed path stays roughly
  21× below the 894,080 marker.
- The two shares come from different wallets and are funded separately: the
  operator keypair must hold the 1.234 SOL tree rent plus fees before step 1,
  and the treasury must hold the 2.666 SOL bitmap rent before step 2, or the
  instruction fails atomically with `TreasuryUnderfunded` and the registry is
  left on the old generation. While compression is enabled, `withdraw_treasury`
  holds that 2.666 SOL back: its reserve is the next token settlement's refund
  **plus** the next generation's four occupancy bitmaps, so the one
  operator-controlled drain on the treasury cannot spend next rotation's rent.
  Task 1.5 carries the instruction change, Task 4.1's step 2 and Task 4.2's
  `DEV.md` entry record the runbook, and the README treasury section states the
  floor.
- **Rotation rent is held back from withdrawals, not merely documented (review
  2026-11-22).** The alternative — the reserve stays at one settlement's refund
  and the operator tops the treasury up when the fill alert fires — puts the
  failure at the worst moment: a rotation is triggered by volume, so an
  unfunded treasury refuses compressed receipts exactly when demand is highest,
  and the fix (transfer SOL, retry step 2) is manual. Holding the rent back
  moves the cost to withdrawal time: `Config::admin` cannot take fees below
  `settlement reserve + 2.666 SOL`, so the treasury accumulates the rent whether
  or not anyone remembers, and only the post-rotation balance needs attention.
  The floor is a check, not a second account: the lamports stay in the treasury
  PDA, and the only thing it binds is the withdrawal path. Settlement refunds
  are the one drain it cannot stop — a token settlement removes `payment_rent`
  and never repays it — which is why the fill/balance alert still watches the
  pool rather than assuming the floor holds it forever. While
  `compression_enabled` is false the floor is zero, so today's live behaviour
  (one settlement's refund plus the account's own rent) is unchanged and the
  0.0303 SOL treasury is not newly frozen.
- Epoch 0 holds **65,536 claims** (8 pages × 8,192 slots). It is not a
  16.7M-slot table: the earlier **683 SOL** figure was the price of a
  fully-grown 2,048-page table sized for the entire lifetime of the protocol,
  and it is only ever paid if volume actually reaches 16.7M invoices.
- Each epoch quadruples the page count (8 → 32 → 128 → … → 2,048). Because
  pages scale with capacity, rent per receipt stays flat at ~40,700 lamports;
  the growth only decides *when* the operator pays it. Every page is rent-paid
  in full whether it is empty or full, so growth is driven by actual fill, and
  the CLI prints the next epoch's rent before creating it.
- Break-even against the legacy marker: `6.57 SOL / (894,080 − 41,650) ≈ 7,700
  settled invoices`. After that every settlement saves ~852,000 lamports of
  permanently locked rent, which covers the 3.900 SOL of a rotation at ~2.5M
  receipts hundreds of times over. The first rotation is not needed until well
  inside generation 0, long after the plan has paid for itself.
- While the flag is off nothing is created and today's 894,080-per-invoice
  cost is unchanged; the shared accounts are created only when the compressed
  path is enabled (rollback keeps them funded but stops adding to them).

## Architecture Decisions

- **Four trees, depth 20, buffer 64, canopy 8.** `[20, 64]` is a valid
  SPL depth/size pair; canopy 8 leaves a `(20 - 8) * 32 = 384`-byte proof, which
  fits comfortably in a transaction. Each tree is 60,600 bytes and costs about
  **0.3085 SOL rent on current mainnet**; four cost about **1.234 SOL once**.
  Capacity is `4 * 2^20 = 4,194,304` receipts per generation at ~294 lamports
  of tree rent each, before any nullifier-state cost.
- **Shard selection is independent of the leaf index.** Derive
  `h = keccak256(invoice_bytes)`; `shard = h[0] >> 6` (top 2 bits),
  `leaf_index = u32::from_le_bytes(h[1..5]) & 0x000F_FFFF` (low 20 bits).
  Using `leaf_index & 3` instead would leave only `2^18` slots per tree.
- **Registry PDA, read-only on the hot path.** `CompressionRegistry` holds
  `current: [Pubkey; 4]`, `previous: [Pubkey; 4]`, `generation: u64`,
  `previous_generation: u64`, `previous_valid_until_slot: u64`, `authority`,
  `bump`, `nullifier_epoch: u64`, `previous_nullifier_epoch: u64`,
  `nullifier_page_count: u16` (power of two, `<= 2,048`), and
  `nullifier_epoch_valid: bool`. Settlement loads it read-only, verifies the tree
  account passed in the instruction equals `current[shard]` or `previous[shard]`
  and that the rotation window is still open, then uses the matching generation
  for occupancy accounts and the active epoch for nullifier pages. No single
  global writable counter exists.
- **Global tagged nullifier pages are the replay guard, sized per epoch.** With
  `h = keccak256(invoice)`, derive the five values from disjoint byte ranges:
  `shard = h[0] >> 6`, `leaf_start = u32le(h[1..5]) & 0xF_FFFF`,
  `page_id = u16le(h[5..7]) & (page_count - 1)`,
  `start_slot = u16le(h[7..9]) & 0x1FFF` (8,192 slots per page), and
  `tag = h[24..32]` (8 bytes). Each slot stores that tag; empty is all zeroes.
  The page PDA is `["nullifier-page", nullifier_epoch:u64, page_id:u16]`.
  `page_count` lives in the registry: **8 pages in epoch 0** (65,536 claims,
  2.67 SOL), quadrupling per epoch to a 2,048-page ceiling (683 SOL) that is
  reached only at 16.7M invoices. The program probes at most 128 slots with
  wraparound in that page: own tag → `AlreadySettled`, foreign tag → continue,
  empty → claim. Pages persist across tree rotations.
- **Rent is paid for the volume that exists, not for the lifetime ceiling.**
  Pages cost the same whether they are empty or full (a page is 65,544 bytes
  regardless of fill), so there is no "early volume is proportionally cheaper"
  effect: epoch 0 costs its 2.67 SOL as soon as the path is enabled. What the
  epoch scheme buys is that the other 680 SOL is never paid unless 16.7M
  invoices really happen. 8-byte tags make a false positive inside an
  8,192-slot page a ~`1e-10` birthday event, so an invoice can never be
  permanently blocked by a tag collision; there are no false negatives.
- **Per-generation occupancy bitmaps allocate tree slots.** Four accounts per
  generation, one per shard, each `8 + 2^20 bits = 131,080` bytes (~0.667 SOL,
  ~2.67 SOL per generation). The program recomputes the shard and starting leaf
  from `h` (top 2 bits / low 20 bits), probes at most 128 bits, requires the
  supplied `leaf_index` to be the first unset bit, and sets it atomically with
  the tree write. Rotation creates fresh occupancy accounts; old ones remain for
  historical verification. These four accounts are the second-largest line
  (2.67 SOL per generation) and are deliberately kept: folding the leaf index
  into the nullifier slot instead would save that rent but couples replay
  ordering to slot allocation. It is recorded in Open Questions as a possible
  follow-up, not part of this plan.
- **Prepared init, never append.** The full sequence is
  `prepare_batch_merkle_tree(20, 64)` → `append_canopy_nodes` (until canopy 8 is
  filled with the empty-tree nodes) → `initPreparedTreeWithRoot` at
  `rightmost_index = 2^20 - 1`, `rightmost_leaf = EMPTY`, `root = empty root`,
  with the 12 non-canopy proof nodes passed as remaining accounts. This sets
  rightmost to `2^20`, making every valid 20-bit index writable. `append` would
  immediately return `TreeFull` and is not used. The tree account is created
  with 60,600 bytes owned by the compression program before this sequence.
- **Proof nodes are remaining accounts, not instruction data.** `replaceLeaf`
  data is exactly discriminator + `root[32] previous_leaf[32] new_leaf[32]
  index:u32` (108 bytes). The 12 proof nodes for depth 20 / canopy 8 are
  read-only remaining accounts after `noop`; SPL reads their pubkeys as node
  bytes and fills the other 8 from the canopy. `verifyLeaf` and
  `initPreparedTreeWithRoot` use the same remaining-account convention.
  Transaction-size checks must count those 32-byte accounts: 384 bytes for one
  proof, so multiple proof CPIs need a larger canopy or a different design.
- **Leaf value is the invoice pubkey (32 bytes).** No time-dependent fields in
  the leaf. The full receipt stays in `PaymentSettled` and the indexer. After
  the nullifier and occupancy claims succeed, the program requires
  `previous_leaf == EMPTY` and `new_leaf == invoice` before CPI. A stale EMPTY
  proof against an occupied slot fails in SPL with `LeafContentsModified`; the
  client/API maps that to a stale-plan retry. Passing a non-EMPTY
  `previous_leaf` is rejected by our program.
- **API proposes slots; the program verifies them.** The API probes the tagged
  nullifier page and the generation's occupancy bitmap to build a plan, but the
  contract recomputes both probe sequences and rejects any plan that skips an
  empty slot or claims an occupied one. The actual `(tree, generation,
  leaf_index)` is still stored in `settled_payments` so status reads are exact.
- **One replay rule for both modes.** Both settle paths check the legacy marker
  PDA and the current/previous nullifier epoch before any transfer. If either
  says settled, the instruction fails with `AlreadySettled`. Both paths then
  claim a nullifier slot; only the compressed path writes a tree leaf, and only
  the legacy path creates the marker. This is what makes flag flips safe:
  pre-upgrade invoices are protected by their marker, and post-upgrade invoices
  are protected by the nullifier regardless of which mode wrote them.
- **Receipt mode is a separate PDA, not a new `Config` field.** `ReceiptMode` at
  `["receipt-mode"]` stores `compression_enabled: bool` and `bump`. An admin-only
  setter checks `Config::admin`; settlement reads the account read-only. This
  follows the `FeeAnchor` precedent and avoids a `CONFIG_SPACE`/`CONFIG_VERSION`
  bump, realloc migration, and changes to `useFeeConfig.ts`'s fixed offsets. The
  IDL still needs regeneration, and the web quote reads this PDA in addition to
  `Config`. Missing account means legacy mode.
- **New `ReceiptCommitted` event** carries `invoice`, `tree`, `generation`,
  `leaf_index`, and `leaf`, emitted alongside the existing `PaymentSettled`. This
  keeps old decoders working and makes a receipt verifiable without the private
  DB.
- **Rotation is two steps: the CLI prepares the trees, the instruction rewires
  the registry.** The admin instruction cannot create trees — the compression
  program requires a signing SPL authority to create and prepare them, and that
  authority exists off chain — so the tree CLI from Task 1.2 always runs first
  and the instruction only validates and rewires. Sequence:
  (1) `tree rotate --prepare` creates four new tree accounts with the same
  parameters, runs prepare/canopy/init, and pays **1.234 SOL** of rent from the
  **operator keypair** (the `Config::admin` signer, which holds SPL tree
  authority until setup completes);
  (2) `rotate_generation` creates the four occupancy bitmaps, validates each new
  tree's owner, header and authority, points `current` at them, moves the old
  addresses to `previous`, increments `generation`, and sets
  `previous_valid_until_slot = clock.slot + 900`.
  The occupancy bitmaps are program PDAs, so only the program can create them:
  the instruction pays their **2.666 SOL** rent from the **treasury** with
  `invoke_signed` and fails with `TreasuryUnderfunded` instead of leaving a
  half-rotated generation. While compression is enabled `withdraw_treasury`
  reserves that rent, so the admin cannot withdraw it out of the pool between
  rotations (Task 1.5). The operator's 1.234 SOL is protocol infrastructure
  and is not reimbursed automatically, so the runbook requires ~1.5 SOL of
  headroom; rotations are scheduled from the fill alert, so the spend is known
  in advance. After the cutoff, `clear_previous` rejects plans built for the old
  generation with `RotationExpired` and the client must ask the API for a fresh
  plan. Old trees and occupancy bitmaps stay funded and verifiable forever; that
  retention policy is documented in `DEV.md` and `README.md` and is part of the
  protocol's shared infrastructure cost.
- **Fee decision (recorded 2026-11-22): tiered percentages only on the
  compressed path.** Drop the receipt-rent floor for compressed receipts. The
  fee is `100 bps` up to the existing `sol_tier_lamports` /
  `token_tier_base_units` boundary and `50 bps` above it; SOL no longer subtracts
  `receipt_rent`, tokens no longer charge a rent-derived flat, and the
  receipt-rent refund is removed, so the treasury keeps the full fee. The token
  invoice-rent refund remains: compressed token settlements still call
  `ensure_treasury_funded` and pay back `payment_rent`, but not
  `payment_rent + receipt_rent`. The flat fields and price-refresh instructions
  stay intact for the legacy marker path until rollback support is dropped.
  Small compressed invoices now pay 1% instead of the old rent floor; the
  "break-even" section of `README.md` is removed when this ships.
- **Raw CPI, not the Rust crate.** `spl-account-compression` 0.4.2 pins
  `anchor-lang 0.29` / `solana-program <=2`, while this program pins Anchor 1.2.1
  and solana-program-test 3.1.10. The compression program is a *deployed*
  program, so we call it with `invoke_signed` and fixed instruction bytes; adding
  the crate is not required and downgrading Anchor is not part of this plan.
  Task 0.2 confirms the deployed program and bytes on devnet.
  Options considered for the version mismatch:
  1. **Raw CPI from the current program (chosen).** No crate dependency, no
     Anchor coupling, works on 1.2.1 and future upgrades.
  2. Add `spl-account-compression` with `cpi` in `Cargo.toml` (rejected): it and
     `spl-concurrent-merkle-tree` pin `solana-program <=2` / Anchor 0.29; mixing
     those types with Anchor 1.2 contexts is fragile and buys nothing over (1).
  3. Write our own Anchor 1.2 sparse-tree program (rejected): full control but
     means reimplementing and auditing Merkle verification and concurrency.
  4. Switch compression stacks (rejected): a different accumulator model would
     change proof/verification design for no benefit here.
  5. Downgrade Anchor (rejected by product).
- **Proofs are generated in the Rust API with `sha3`, not in a JS sidecar or via
  the SPL Rust crate.** A proof is ~20 keccak256 hashes of 32-byte nodes —
  microseconds, dwarfed by the RPC and libSQL round trips — so "faster" is about
  process topology, not CPU. `sha3` adds 14 crates; `spl-concurrent-merkle-tree`
  0.4.1 does compile alongside `solana-pubkey 3` (verified with `cargo check`)
  but pulls 397 crates including `solana-program 1.18` and the ark/poseidon
  stack. A Node sidecar would add a second runtime to the Vercel Rust function.
  The local builder mirrors SPL hashing exactly (`keccak256(left || right)` with
  cached empty-node hashes) and is pinned by vectors cross-checked against the
  JS SDK during Task 3.2.
- **Rust owns everything the browser cannot know or safely decide.** The API
  chooses the shard, probes nullifier tags and occupancy bits, returns the fresh
  `root`, `previous_leaf`, `new_leaf`, tree/generation, nullifier page, and
  changelog-aware retry data, and resolves/validates compressed receipt status.
  The contract re-checks both probe sequences on chain. The browser keeps wallet
  connection, signing and sending, plus ordinary PDA derivation; it must never
  derive a compressed receipt location by itself. Concretely,
  `useCheckoutChain.ts`'s direct `getAccountInfo(receiptPda)` fallback is replaced
  by an API read, and both transaction entry points consume the same Rust plan:
  the in-app checkout gets a `settlement-plan` response, while Solana Pay consumes
  it through the builder rebuilt in Task 3.4.
- **The client fee quote stays in TS but must mirror the active Rust/on-chain
  path.** `app/utils/invoiceCost.ts` is intentionally a pure mirror of `fee_for`,
  with tests pinning it to `helpers.rs`. It must read the compressed-path flag
  from the `ReceiptMode` PDA: with the flag off it keeps today's rent floor and flats;
  with the flag on it drops `receiptRent`, `recordFee`, `feeIsFloor` and ignores
  the flats. No JS service is introduced. If drift becomes a problem, a Rust
  quote endpoint is the follow-up, not a JS source of truth.
- **Compression program ID is cluster-specific and gated.** The noop program is
  `noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`. Devnet's canonical compression
  ID is `cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK`; mainnet's canonical ID
  does not expose prepared-init instructions, so the Compatibility Gate above
  applies. `replaceLeaf` accounts are `merkleTree` (writable), `authority`
  (signer), `noop`, then proof nodes as read-only remaining accounts; data is
  discriminator `[204,165,76,100,73,147,0,128]` plus
  `root[32] previous_leaf[32] new_leaf[32] index:u32`.

## Issues Addressed

| ID | Issue | Severity | Phase |
| --- | --- | --- | --- |
| 1 | 894,080 lamports locked per settled invoice | High | 1–3 |
| 2 | Fixed capacity / no reclaim path for the receipt account | Medium | 4 |
| 3 | Single hot account would have shattered shard parallelism | High | 1 |
| 4 | Fee floor and token flats depend on the disappearing receipt rent | High | 0, 2 |
| 5 | Receipt must remain verifiable without the private DB | Medium | 2, 3 |
| 6 | Shared-tree slot probing must not become a replay hole | High | 1, 2, 4 |

## Dependencies

- Product decision (resolved in Task 0.1 for the compressed path): tiered
  percentages only, no receipt-rent floor, no rent-derived flats, no
  receipt-rent refund. Token invoice rent is still refunded. Legacy path keeps
  its current math.
- Replay design (resolved in the review decision above): tagged global nullifier
  pages plus per-generation occupancy bitmaps; no per-invoice account. Both modes
  claim the nullifier and check the legacy marker fallback.
- Technical decision (default set in Task 0.2): raw CPI; the Rust crate is not a
  prerequisite and no Anchor downgrade is planned.
- `plans/0002` must be deployed; coordinate with `0004` and `0005`, which touch
  the same fee math and refund paths.
- Task 0.2 verifies devnet bytes; Task 0.3 is a hard gate for mainnet because the
  canonical mainnet compression program lacks prepared-init instructions. No
  compressed flag may be enabled on a cluster until that gate passes there.
- Receipt mode is a separate `ReceiptMode` PDA, not a `Config` byte; no config
  realloc or fixed-offset reader migration is part of this plan.

## Phases

### Phase 0: Decisions and compatibility spike

#### Task 0.1: Record the fee decision (resolved)

Description: Decision recorded 2026-11-22: on the compressed path use the tiered
percentages that were already specified — 1% (100 bps) up to the existing
boundary and 0.5% (50 bps) above it — with no receipt-rent floor. This means
`fee_for` SOL = `proportional` with no `saturating_sub(receipt_rent)`,
`fee_for` tokens = `proportional` with no flat, and no per-invoice treasury
refund of the receipt rent. Token settlements still refund the invoice's
`payment_rent` and keep `ensure_treasury_funded(payment_rent)`. The two flat
fields, `refresh_fee_price`, and `set_fee_price_anchor` stay unchanged because
they are still needed by the legacy marker path; the compressed path simply
ignores them. Legacy markers keep their existing accounting until the flag is
flipped.

Acceptance criteria:

- [x] This plan and the Architecture Decisions section state the decision and
      its consequences for the SOL deduction, token flats, and treasury refund.
- [x] Flat fields and price-refresh instructions remain active for the legacy
      path; only the compressed path ignores them. *(Confirmed 2026-11-26: the
      legacy fields and `refresh_fee_price`/`set_fee_price_anchor` are still live;
      there is no compressed path yet.)*
- [x] Small-invoice behaviour is explicit: 1% of the amount, not a rent floor.
- [x] The fee decision and replay design are recorded. `README.md` fee prose is
      updated in Task 5.2, not before the feature ships.

Verification:

- [ ] Review with human; no code change in this task.

Files likely touched:

- `plans/deferred/0006-compressed-receipts.md` (done), `README.md` (Task 5.2)

Dependencies: None. **Resolved. Phase 2 still depends on the compatibility gate
and the on-chain foundation tasks.**

Estimated scope: Small.

#### Task 0.2: Verify SPL compression integration and cluster availability

Description: Prove that a depth-20/buffer-64/canopy-8 tree can be created and a
`replaceLeaf` CPI executed from this program on devnet using the raw-CPI path,
and record the real cluster compatibility facts. No crate dependency and no
Anchor downgrade are part of this task.

Acceptance criteria:

- [ ] A throwaway devnet test creates a 60,600-byte account owned by the
      compression program, runs `prepare_batch_merkle_tree(20, 64)` →
      `append_canopy_nodes` → `initPreparedTreeWithRoot`, then writes one leaf
      via `invoke_signed`; the discriminator, account order, 12 proof-node
      remaining accounts, and 108-byte data layout are confirmed.
- [ ] A compatibility note records that the deployed program is called as an
      external program, so Anchor 1.2.1 stays unchanged.
- [ ] The 36-byte program account bytes, the programdata address, and the
      **programdata account** length/hash are recorded separately for devnet and
      mainnet. The old "matching `5077349f…c5b0a3`" claim is removed: that hash
      was of the 36-byte program account, not the executable bytes.
- [ ] The tree account length is **computed, not hard-coded**: call
      `getConcurrentMerkleTreeAccountSize(20, 64, 8)` from the SPL JS/Rust
      helper, record the result (60,600) and pin it as a constant with a test
      that fails if the SDK value differs. Rent is then
      `getMinimumBalanceForRentExemption(len)` from live RPC (308,498,240
      lamports today), not a typed-in number.
- [ ] Program IDs, the `[20, 64]` pair, the tree length and its rent are written
      to `DEV.md` from live RPC output.

Verification:

- [ ] Focused devnet test passes; `task verify` passes.

Files likely touched:

- `apps/contracts/programs/payvand/Cargo.toml`, a scratch test, `DEV.md`

Dependencies: None.

Estimated scope: Medium.

#### Task 0.3: Provision a mainnet-compatible compression program

Description: Mainnet's canonical compression program does not expose the
prepared-init instructions. Deploy or arrange an upgrade to a reviewed
SPL-compatible build that exposes `prepare_batch_merkle_tree`,
`append_canopy_nodes`, `initPreparedTreeWithRoot`, `replaceLeaf`, and
`transferAuthority`, then record its program ID per cluster. This is a hard gate
for mainnet rollout, not for devnet Phase 1, which can use the canonical devnet
program. Without a compatible program a cluster cannot initialize trees for
arbitrary 20-bit indices.

Acceptance criteria:

- [ ] A full prepare/canopy/init/replaceLeaf smoke run succeeds on mainnet with
      the chosen program ID; a devnet-only result is not sufficient.
- [ ] `DEV.md` records the program ID, programdata length/hash, noop ID, and
      exact instruction discriminators used by the raw CPI wrappers.
- [ ] Rollback verifies that no mainnet tree is created with a program lacking
      prepared-init support.

Verification:

- [ ] Signed mainnet smoke run appended to this plan; `task verify` passes.

Files likely touched:

- `DEV.md`, `Taskfile.yml`, deployment scripts, this plan

Dependencies: 0.2.

Estimated scope: Large / external coordination.

#### Task 0.4: Record the product policy in `README.md` (done in review)

Description: `README.md` now states the planned compressed-receipt policy before
implementation: shared-tree commitment instead of a per-invoice account, 100/50
bps only, no receipt-rent floor/flat/refund, protocol-funded shared accounts,
indefinite verification of rotated trees, and a legacy rollback path. The live
accounting section stays unchanged until the flag is enabled.

Acceptance criteria:

- [x] `README.md` contains the planned policy and marks it as not yet shipped.
      *(Verified 2026-11-26 at `README.md:454-475`.)*
- [x] This plan no longer defers the policy decision to Task 5.2; Task 5.2 only
      updates the prose from "planned" to "live".

Verification:

- [ ] Review only; no code change in this task.

Files likely touched: `README.md` (done), `plans/deferred/0006-compressed-receipts.md`.

Dependencies: 0.1. **Resolved.**

Estimated scope: Small.

### Phase 1: On-chain foundation

#### Task 1.1: Add the `CompressionRegistry` account and init instruction

Description: Define the PDA (e.g. `["compression-registry"]`), the
`current`/`previous` arrays, generation fields, and an admin-only initializer.
Add the account to the IDL and to the settle context as read-only.

Acceptance criteria:

- [ ] Registry can be initialized once with four tree pubkeys, generation 0,
      `previous_generation = 0`, `previous_valid_until_slot = 0`, nullifier
      epochs `0`/`0`, `nullifier_page_count = 8`, and
      `nullifier_epoch_valid = false`; a second init fails, and a page count
      that is not a power of two or exceeds 2,048 is rejected.
- [ ] Only `Config::admin` can write the registry through its instructions.
- [ ] Settle instructions can read it without making it writable.

Verification:

- [ ] Contract unit test covers init, duplicate init, and unauthorized write.
- [ ] `task verify`, `task anchor:test`.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/contracts/programs/payvand/src/lib.rs`

Dependencies: 0.2.

Estimated scope: Medium.

#### Task 1.2: Tree creation tooling and prepared initialization

Description: Add a Rust CLI/bin (not another web `scripts/*.mjs`) that creates
four trees with depth 20, buffer 64, canopy 8, with two subcommands over one
implementation: `tree create` for a first-time set and `tree rotate --prepare`
for the next generation's four trees, which is the step Task 4.1's on-chain
instruction depends on (it can only rewire the registry; it cannot create or
prepare trees). The CLI keypair remains the SPL authority while it runs
`prepare_batch_merkle_tree(20, 64)` → `append_canopy_nodes` →
`initPreparedTreeWithRoot` at `2^20 - 1`, so it can sign every setup
instruction. Only after the header is fully initialized does it call
`transferAuthority` to the program's tree-authority PDA. It uses the same fixed
instruction bytes as the on-chain raw CPI path; the JS SDK is only a test-vector
reference.

Acceptance criteria:

- [ ] Running the script against devnet yields four tree addresses whose
      headers report maxDepth 20, maxBufferSize 64, canopy 8, and the tree-authority
      PDA as the final authority.
- [ ] No setup instruction is attempted after authority has moved to the PDA;
      the script asserts each header's authority at the point each instruction
      is signed.
- [ ] A leaf written by the program after the transfer succeeds with
      `invoke_signed`, and a CLI-signed `replaceLeaf` fails because the CLI is no
      longer authority.
- [ ] A proof for an arbitrary 20-bit index verifies against the empty root,
      confirming every slot is writable.
- [ ] Addresses are recorded in `DEV.md` and wired into the registry.
- [ ] `tree rotate --prepare` exists and is the same preparation path as
      `tree create`: it creates four new accounts, runs the full prepare/canopy/
      init sequence, transfers authority to the tree-authority PDA, leaves the
      existing generation untouched, and prints the four addresses plus
      signatures in the form `rotate_generation` (Task 4.1 step 2) consumes.

Verification:

- [ ] Devnet smoke run; addresses and signatures pasted into the plan or `DEV.md`.

Files likely touched:

- `apps/api/src/bin/` (new tree CLI), `apps/api/Cargo.toml`, `DEV.md`, `Taskfile.yml`

Dependencies: 0.2, 1.1; additionally 0.3 when running against mainnet.

Estimated scope: Medium.

#### Task 1.3: Raw CPI module for `replaceLeaf`

Description: Implement the local CPI wrapper with explicit account and
parameter validation: tree owner is the SPL compression program, header matches
the expected parameters, proof path length is `depth - canopy`, authority is the
expected PDA, and the noop program is the known address.

Acceptance criteria:

- [ ] A leaf can be replaced from a test program after authority has been
      transferred to the PDA; wrong owner, wrong header, wrong proof length, or
      wrong authority all fail before CPI.
- [ ] A CLI/caller-signed replace after the transfer is rejected, proving the
      authority check is on the PDA and not the setup keypair.
- [ ] The changelog event is observable in the transaction logs for indexing.

Verification:

- [ ] Contract test for each rejection case; `task verify`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/` (new module)
- `apps/contracts/programs/payvand/src/helpers.rs`

Dependencies: 0.2, 1.2; additionally 0.3 when targeting mainnet CPI.

Estimated scope: Medium.

#### Task 1.4: Tagged nullifier pages and generation occupancy bitmaps

Description: Add the two on-chain state types that make replay and collision
probing authoritative. A `NullifierPage` is a PDA of
`["nullifier-page", nullifier_epoch:u64, page_id:u16]` with 65,544 bytes: an
8-byte header and 8,192 8-byte tag slots. **Epoch 0 has 8 pages** (65,536
claims, 2.67 SOL); each later epoch quadruples the count (32, 128, … up to the
2,048-page ceiling). `page_count` is read from the registry, always a power of
two, so `page_id` is a mask rather than a modulo. An `OccupancyBitmap` is a PDA
of `["tree-occupancy", generation:u64, shard:u8]` with 131,080 bytes: an 8-byte
header and one bit per leaf. Add permissionless-on-demand init instructions that
create the correct PDA with program ownership, plus read/write helpers that
validate the PDA, slot bounds, and probe order. The Rust API CLI pre-creates
pages and generation bitmaps; settlement only uses already-initialized accounts,
and the CLI prints the rent of the next page/epoch before creating it.

Acceptance criteria:

- [ ] `keccak256(invoice)` yields shard, leaf start, page id, start slot and tag
      from disjoint byte ranges (`h[0] >> 6`, `u32le(h[1..5]) & 0xF_FFFF`,
      `u16le(h[5..7]) & (page_count - 1)`, `u16le(h[7..9]) & 0x1FFF`,
      `h[24..32]`), with pinned test vectors; the page PDA includes the active
      nullifier epoch.
- [ ] `page_count` is validated as a power of two and `<= 2,048`, and a plan
      built for a different epoch is rejected before any state write.
- [ ] A claim scans at most 128 slots/bits with wraparound, finds its own tag as
      `AlreadySettled`, skips only foreign tags, and writes only the first empty
      slot/bit.
- [ ] A page/bitmap with a wrong PDA, wrong generation, wrong shard, or wrong
      size is rejected before any state write or CPI.
- [ ] Page and bitmap sizes and rent-exemption amounts are confirmed against
      live RPC (65,544 B → 333,613,760 lamports; 131,080 B → 666,536,640
      lamports) and written to `DEV.md` with the epoch-0 page count.

Verification:

- [ ] Contract unit tests for claim, replay, foreign-tag skip, wraparound, full
      page/bitmap, and each rejection case; `task verify`.
- [ ] A devnet script creates one page and four generation-0 bitmaps and prints
      their addresses/signatures.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/instructions/compression.rs` (new)
- `apps/contracts/programs/payvand/src/helpers.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/api/src/bin/` (tree/state CLI), `DEV.md`

Dependencies: 1.1, 1.2.

Estimated scope: Medium.

#### Task 1.5: Receipt-mode PDA and admin setter

Description: Add the `ReceiptMode` account at `["receipt-mode"]` with
`compression_enabled: bool` and `bump`, plus an admin-only
`set_compression_mode(enabled: bool)` instruction that checks `Config::admin`.
Both settle contexts read it read-only; a missing account decodes as legacy
mode. Regenerate the IDL and add a fixed-offset web reader beside
`useFeeConfig.ts` without changing `CONFIG_SPACE` or `CONFIG_VERSION`.

The same account is what `withdraw_treasury` reads to keep the next rotation's
rent in the pool. With `compression_enabled` true the withdrawal reserve adds
`4 * Rent::minimum_balance(COMPRESSION_BITMAP_SPACE)` on top of the settlement
refund; the bitmap size constant and the existing `TREASURY_RESERVE_SETTLEMENTS`
keep the arithmetic in one place. `ReceiptMode` is optional and read-only in
that context, so a missing account or `false` leaves the legacy floor byte for
byte as it is today.

Acceptance criteria:

- [ ] Only `Config::admin` can flip the flag; unauthorized setters fail.
- [ ] `withdraw_treasury` reserves the settlement refund **plus** the next
      generation's four occupancy bitmaps (2.666 SOL on mainnet) while
      `compression_enabled` is true, computed from `Rent` and the bitmap size
      constant; with the flag off, or the account missing, the reserve and the
      withdrawable amount match today's exactly.
- [ ] Settle instructions read the flag without making `ReceiptMode` writable.
- [ ] A missing `ReceiptMode` account and `compression_enabled = false` both
      select the legacy marker path; `true` selects the compressed path.
- [ ] The regenerated IDL exposes the account and instruction, and the web
      quote reads the same flag the contract does.

Verification:

- [ ] Contract unit tests for admin check and all three flag states;
      `task verify`, `task anchor:test`, IDL diff reviewed.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs` (or a new mode module)
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/assets/payvand.idl.json`, `apps/web/app/composables/useFeeConfig.ts`

Dependencies: 1.1.

Estimated scope: Small–Medium.

**Checkpoint A:** registry initialized, four devnet trees live, nullifier pages
and generation-0 bitmaps live, receipt-mode flag readable, CPI writes a leaf,
`task verify` and `task anchor:test` pass. Review before touching settlement.

### Phase 2: Settlement path with a dual replay guard

#### Task 2.1: Add compressed accounts to the settle instructions

Description: Extend both SOL and token settle instructions with the compressed
accounts (registry, `ReceiptMode`, tree, tree authority, noop, one nullifier page
for the active epoch, one occupancy bitmap for the validated generation/shard)
and emit `ReceiptCommitted`. Keep the legacy `PaymentReceipt` marker path intact;
select the path with the `ReceiptMode` PDA so a rollback is an admin write. In
both modes, check the marker PDA and nullifier before transferring, and claim a
nullifier slot; the marker remains readable as the fallback for pre-upgrade
invoices.

Acceptance criteria:

- [ ] With the flag off, settlement still writes the legacy marker and today's
      `PaymentSettled` event. The legacy path must **not require** a nullifier
      page: it always checks the marker, and only checks/claims a tag when the
      page is passed and already initialized. A missing page never blocks a
      legacy settlement, so a flag flip cannot become an outage and a stranger
      cannot grief settlements by never creating pages.
- [ ] With the flag on, no new receipt account is created and
      `ReceiptCommitted` contains invoice, tree, generation, leaf_index, and leaf.
      The legacy marker account is still passed read-only for pre-upgrade replay
      checks.
- [ ] The receipt account is no longer `#[account(init_if_needed)]` on the
      compressed path: it becomes an optional read-only account used only for
      the pre-upgrade replay check, so a compressed settle neither creates nor
      requires it. The legacy path keeps its current init behaviour.
- [ ] Exactly one occupancy bitmap is accepted: the one matching the validated
      generation and shard. Missing, duplicate, or extra bitmaps are rejected.
- [ ] Existing IDL consumers still decode `PaymentSettled` unchanged.

Verification:

- [ ] Contract tests for both flag states and each bitmap-count rejection;
      `task verify`, `task anchor:test`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/events.rs`
- `apps/contracts/programs/payvand/src/state.rs`
- `apps/web/app/assets/payvand.idl.json`

Dependencies: 1.1, 1.3, 1.4, 1.5.

Estimated scope: Large — split into SOL and token subtasks if it exceeds a session.

#### Task 2.2: Enforce the nullifier rule and rotation acceptance

Description: Implement the tagged-nullifier claim on both settle modes, the
legacy-marker fallback check, the compressed occupancy-bitmap claim, and the
leaf-state rule (`EMPTY` → write, foreign value → reject). Accept trees in either
`current[shard]` or `previous[shard]` only while
`clock.slot <= previous_valid_until_slot`, using the matching generation's
occupancy bitmap. All claims, the transfer, and the CPI are one atomic
transaction.

Acceptance criteria:

- [ ] A second settlement of the same invoice fails with `AlreadySettled` in
      both modes, even if it targets a different slot, tree, or generation.
- [ ] An invoice settled before the mode flag existed is rejected by its legacy
      marker when the same invoice is later submitted in compressed mode.
- [ ] An invoice settled in compressed mode is rejected by its nullifier when
      the flag is rolled back to legacy mode.
- [ ] Off→on and on→off flag flips each have tests for both the previously
      settled and the not-yet-settled case.
- [ ] A supplied `leaf_index` that is not the first unset bit in the matching
      occupancy bitmap is rejected.
- [ ] A tree address not in `current` or `previous`, an occupancy bitmap from
      the wrong generation, or a plan past `previous_valid_until_slot` is
      rejected; the last returns `RotationExpired` and the client must refresh.
- [ ] A still-valid plan built against a `previous` tree succeeds inside the
      rotation window; a stale plan whose occupied bit changed fails with a
      retryable error.

Verification:

- [ ] Contract tests for replay across slots/trees/generations/modes, wrong
      shard, wrong generation, stale plan, both rotation-window edges, and all
      four flag transitions; `task verify`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/errors.rs`

Dependencies: 2.1.

Estimated scope: Medium.

#### Task 2.3: Switch the fee math to the decided cost basis

Description: Apply Task 0.1's decision on the compressed path: fee is the tiered
percentage only (100 bps to the boundary, 50 bps above), with no receipt-rent
subtraction, no rent-derived flat, and no receipt-rent refund. The token
invoice-rent refund stays. Legacy marker path keeps today's math until the flag
is flipped.

Acceptance criteria:

- [ ] Compressed SOL fee = `proportional`; compressed token fee =
      `proportional`, with both flat fields ignored even when the legacy path
      still writes them.
- [ ] Compressed settlement advances no `receipt_rent` and refunds none; the
      treasury keeps the full percentage fee.
- [ ] Compressed token settlement still calls `ensure_treasury_funded` and
      refunds `payment_rent` only. The `TreasuryUnderfunded` guard remains, with
      `payment_rent` as its amount, and reserve/withdraw docs are updated.
- [ ] The treasury's funding docs cover the rotation floor from Task 1.5, not
      only settlement refunds: while compression is enabled `withdraw_treasury`
      keeps the next 2.666 SOL of bitmap rent in the pool, so the docs say the
      treasury is funded to `2.666 SOL + the settlement reserve` and that the
      admin cannot withdraw that rent; settlement refunds are the remaining
      drain and are what the balance alert watches; a treasury short anyway
      fails the rotation atomically instead of half-applying it; and the
      operator holds a separate 1.234 SOL for the trees.
- [ ] Invoice-too-small guards use `payment_rent + fee` (no receipt rent) on the
      compressed SOL path; the token path keeps its existing `amount > fee`
      assertion. `seller_proceeds` in the event is `amount - fee`.

Verification:

- [ ] Updated accounting assertions in `tests/settle.rs`; `task verify`.

Files likely touched:

- `apps/contracts/programs/payvand/src/helpers.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/tests/settle.rs`

Dependencies: 0.1, 2.1.

Estimated scope: Medium.

**Checkpoint B:** both settlement paths work on devnet, replay is enforced by the
tagged nullifier plus legacy-marker fallback, all four flag transitions are
tested, and fee deltas match the recorded decision. Human review before API work.

### Phase 3: Indexer, proof service, and status reads

#### Task 3.1: Extend the settlement schema

Description: Add `receipt_kind`, `tree`, `generation`, `leaf_index`, `leaf`,
`nullifier_page`, and `nullifier_slot` to the settlement record, nullable for
legacy markers. Add a libSQL migration that backfills nothing and leaves old
rows untouched.

Acceptance criteria:

- [ ] Existing rows decode exactly as before.
- [ ] New compressed rows carry enough data to rebuild a proof later.

Verification:

- [ ] `task test:api`; migration runs twice without error.

Files likely touched:

- `apps/api/src/db.rs`, migration files, `apps/api/src/payments.rs`

Dependencies: 2.1.

Estimated scope: Medium.

#### Task 3.2: Proof service with collision probing and refresh

Description: Build Merkle proofs in Rust (`sha3::Keccak256`, no Node sidecar and
no SPL Rust crate) from the indexer's leaf map plus tree changelogs. Probe the
tagged nullifier page and the generation occupancy bitmap to choose
`(nullifier_slot, leaf_index)`, pre-create a missing nullifier page through the
CLI/instruction rather than silently skipping it, and refresh proofs that fall
out of the 64-entry changelog buffer. Expose one `settlement-plan` endpoint that
returns `tree`, `generation`, `leaf_index`, `nullifier_page`, `nullifier_slot`,
`root`, `previous_leaf`, `new_leaf` and retry metadata, and consume it from the
in-app checkout. Solana Pay consumes the same plan after its builder is rebuilt
in Task 3.4. The browser only adds these accounts to the Anchor instruction and
signs; it does not compute the plan.

Acceptance criteria:

- [ ] The local hash matches SPL's `keccak256(left || right)` and its cached
      empty-node values, checked against vectors produced once with the JS SDK.
- [ ] The settlement-plan response is accepted by the in-app checkout; no JS
      path derives a leaf index or reads tree state.
- [ ] Two invoices whose first 20 hash bits collide settle into different
      slots and both verify; a third submission of either invoice is rejected
      by the tagged nullifier page.
- [ ] A plan targeting a missing nullifier page is rejected until the page is
      created; no invoice is silently reassigned to another page.
- [ ] A proof older than the changelog buffer is refreshed and the retry
      succeeds; the failure message names `RootNotFound`/`LeafContentsModified`.
- [ ] Proof size for canopy 8 is 384 bytes and the transaction stays under the
      size limit.

Verification:

- [ ] API unit tests for collision and stale-proof cases; devnet settle through
      the real endpoint.

Files likely touched:

- `apps/api/src/` (new proof module), `apps/api/src/payments.rs`,
  `apps/api/src/lib.rs`, `apps/web/app/composables/useCheckoutPayment.ts`

Dependencies: 3.1, 1.3, 1.4, 1.5, 2.2.

Estimated scope: Large — split proof-building and retry into two tasks if needed.

#### Task 3.3: Backward-compatible receipt reads

Description: Resolve a settled invoice from, in order, the new compressed row,
the existing `PaymentSettled` event via `settled_from_history`, or a legacy
48-byte marker. In `fetch_payment` and `InvoiceReader::record`, look up the
compressed row by `(invoice, endpoint)` before the existing marker decode; today
`record()` only enters `settled()` when a marker decodes, so compressed invoices
would otherwise show as pending after the invoice account closes. Verify
compressed receipts against the tree root where the proof is available.

Acceptance criteria:

- [ ] Old settled invoices still render with full receipt fields.
- [ ] New invoices render from the event plus the compressed row without a
      marker account, including when the invoice account no longer exists.
- [ ] A missing history entry degrades to the stored row, not to an empty
      receipt.
- [ ] API tests call the real `fetch_payment` path, not only the row decoder,
      for a compressed settlement with no marker.
- [ ] The checkout no longer probes a receipt PDA from the browser; with a
      compressed flag on, `useCheckoutChain.ts` gets settlement status from the
      API, which carries the verified `(tree, generation, leaf_index)` from the row.

Verification:

- [ ] API tests covering legacy, compressed, and history-unavailable cases;
      `task test:api`.

Files likely touched:

- `apps/api/src/payments.rs`, `apps/web/app/composables/useCheckoutChain.ts`

Dependencies: 3.1, 3.2.

Estimated scope: Medium.

#### Task 3.4: Rebuild the Solana Pay transaction builder

Description: `settle_payment_transaction` in `apps/api/src/solana_pay.rs` is
already stale: it hardcodes six keys and account indexes `[0,1,2,3,4]`, while the
current IDL's `settle_payment` requires payer, payment, seller, receipt,
reference, config, treasury, and system program, and the compressed path adds
registry, mode, tree, authority, noop, nullifier page, and one occupancy bitmap.
Rebuild the builder from the generated IDL account list and the settlement plan
instead of extending the old constants. The wallet remains the only signer.

Acceptance criteria:

- [ ] A parsed-wire fixture asserts every account meta and index for both the
      legacy and compressed layouts, including readonly proof nodes if present.
- [ ] The transaction built for a settlement plan matches the IDL account order
      exactly; the old six-key fixture is replaced, not patched.
- [ ] A compressed Solana Pay settlement succeeds end to end on devnet using the
      same plan as in-app checkout.

Verification:

- [ ] API wire-format tests, `task test:api`, and a devnet Solana Pay smoke run.

Files likely touched:

- `apps/api/src/solana_pay.rs`, its tests, `apps/web/app/assets/payvand.idl.json`

Dependencies: 1.3, 1.4, 1.5, 2.2, 3.2.

Estimated scope: Medium.

**Checkpoint C:** a buyer can pay a QR invoice end to end through both in-app
checkout and Solana Pay with no receipt account, and the seller's status screen
shows the full receipt. Old invoices still work.

### Phase 4: Rotation and operations

#### Task 4.1: Rotation instruction and scripts

Description: Rotation is an operator sequence of two steps, because the
admin instruction cannot create trees.

**Step 1 — CLI, off chain, pays from the operator keypair.** The tree CLI from
Task 1.2 (`tree rotate --prepare`) creates four new accounts with depth 20,
buffer 64, canopy 8, runs `prepare_batch_merkle_tree` → `append_canopy_nodes`
→ `initPreparedTreeWithRoot` while the operator keypair is still the SPL
authority, then transfers each tree's authority to the program's tree-authority
PDA. The operator keypair (`Config::admin`, run by whoever operates the
deployment) is the payer and must be a signer: a PDA cannot pay for a
`create_account`, so this step cannot move on chain. Cost is **1.234 SOL**
(4 × 0.3085 SOL), spent once per generation and not reimbursed automatically.
The CLI prints the four addresses, the signatures, and the SOL balance after.

**Step 2 — on-chain instruction, pays from the treasury.** Admin-only
`rotate_generation(new_trees: [Pubkey; 4])` creates the next generation's four
occupancy bitmaps (they are PDAs owned by this program, so only the program can
create them) with the **treasury PDA as payer** via `invoke_signed` — **2.666
SOL** — then validates each supplied tree (owner is the compression program,
header is 20/64/8, authority is the tree-authority PDA, account is not already
in `current`/`previous`), writes them into `current`, moves the old addresses to
`previous`, increments `generation`, records `previous_generation`, and sets
`previous_valid_until_slot = clock.slot + 900`. The whole step is one atomic
transaction: either all four bitmaps exist and the registry points at the new
generation, or nothing changes. A treasury short of 2.666 SOL fails with
`TreasuryUnderfunded` before any account is created. A `clear_previous`
instruction refuses to run before that slot, then clears `previous` and
`previous_generation`. Add `pnpm` / `Taskfile` commands for both steps.

Acceptance criteria:

- [ ] The CLI prepares all four trees and prints their addresses before step 2
      is submitted; step 2 rejects an address that is unprepared, unowned, or
      has the wrong header/authority, so an operator typo cannot brick a
      generation.
- [ ] The CLI is the payer of the four tree accounts (1.234 SOL) and does not
      run after authority transfer; the treasury is the payer of the four
      occupancy bitmaps (2.666 SOL) and a short treasury fails the instruction
      with `TreasuryUnderfunded` without creating anything.
- [ ] The treasury's 2.666 SOL is already held back by `withdraw_treasury`
      (Task 1.5) when compression is enabled, so step 2 does not depend on the
      admin having left the rent in place; a treasury drained below it by
      settlement refunds still fails atomically and the runbook tops it up.
- [ ] `rotate_generation` with fewer or more than four new trees fails, and a
      re-run against a generation that is already current fails.
- [ ] Rotation can be performed without changing invoice addresses or IDs.
- [ ] New invoices go to new trees and generation bitmaps; old receipts still
      verify against old trees and occupancy bitmaps.
- [ ] Replay of an invoice settled before rotation fails even when the new tree
      has an empty slot at the same leaf index.
- [ ] Before `previous_valid_until_slot`, `clear_previous` fails; after it, a
      plan built for the old trees fails with `RotationExpired` and the API
      returns a fresh plan on retry.
- [ ] `DEV.md` records the two-step sequence, who pays each rent, the required
      operator SOL headroom (~1.5 SOL), and the cutoff slot printed by step 2.

Verification:

- [ ] Devnet rotation test with one settlement before, one inside the window,
      and one after `clear_previous`, plus a rejected step 2 pointed at an
      unprepared tree and a step 2 from a treasury funded below 2.666 SOL.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/contracts/programs/payvand/tests/rotation.rs` (new)
- `apps/api/src/bin/` (tree CLI from Task 1.2: `tree rotate --prepare`)
- `package.json`, `Taskfile.yml`, `DEV.md`

Dependencies: 1.1, 1.2, 1.4, 2.2.

Estimated scope: Medium.

#### Task 4.2: Monitoring and runbook

Description: Alert on per-generation occupancy (rotate at ~60% of a shard to
leave probe headroom), tagged-nullifier page fill and missing-page failures,
proof staleness rate, `LeafContentsModified` failures, and all account rent.
Document rotation, nullifier page pre-creation, and indexer-catch-up procedures.

Acceptance criteria:

- [ ] Dashboard/alerts exist for the four trees, registry, occupancy bitmaps,
      and nullifier pages.
- [ ] `DEV.md` lists tree addresses, authority PDA, nullifier page address
      derivation, the two-step rotation sequence with its per-step rent and
      payer (operator 1.234 SOL for trees, treasury 2.666 SOL for bitmaps,
      3.900 SOL per generation) and the operator headroom, the 900-slot cutoff,
      the `RotationExpired` retry path, and rollback.
- [ ] `DEV.md` and the README treasury section state how a rotation is funded:
      the operator keypair holds its 1.234 SOL, `withdraw_treasury` holds the
      treasury's 2.666 SOL back while compression is enabled so it cannot be
      withdrawn before a rotation, settlement refunds are the remaining drain and
      are what the balance alert watches, the operator confirms both balances
      before step 1 rather than discovering a shortfall at step 2, and each
      rotation leaves another 3.900 SOL permanently locked.
- [ ] `README.md`/`DEV.md` state that rotated trees, occupancy bitmaps, and
      nullifier pages are retained indefinitely, so each rotation permanently
      adds 3.900 SOL; any future reclaim requires a separate decision and
      nullifier backfill.

Verification:

- [ ] Simulate a near-full shard in devnet and confirm the alert path.

Files likely touched:

- `DEV.md`, monitoring config, `apps/api/src/` metrics

Dependencies: 4.1.

Estimated scope: Small.

#### Task 4.3: Nullifier epoch expansion

Description: Epoch 0 holds 65,536 claims in 8 pages (2.67 SOL). `page_count`
lives in the registry, so expansion is an operational procedure, not a program
redesign. Alert at ~70% fill, print the next epoch's rent, then open the next
epoch with **four times** the pages (32 pages, 262,144 claims, 10.67 SOL — rent
per claim stays at ~40,700 because pages scale with capacity, so the operator
only pays for volume that exists). During migration the contract checks both
epochs for the invoice tag and only claims in the new epoch. Copy old tags in
batches into the same page/slot of the new epoch (~0.5 MB for 65,536 8-byte
tags, one transaction per page), verify per-page counts and a full scan, then
make the new epoch current and keep the previous one readable for the in-flight
window. Doubling instead of quadrupling is acceptable if volume is uncertain;
it only changes how often this task runs. This task is required before the
first mainnet rollout.

Acceptance criteria:

- [ ] A replay from any earlier epoch fails after the switch, including when the
      new epoch page is empty at the same slot.
- [ ] Migration never skips a tag; per-page counts and a full scan are verifiable
      before the switch.
- [ ] During migration a settlement checks both epochs atomically with the tree
      write and occupancy claim.
- [ ] Clearing `previous_nullifier_epoch` only happens after the in-flight
      window.
- [ ] The next epoch's rent and page count are printed and approved before any
      page is created, both at ~70% fill of the current epoch.

Verification:

- [ ] Devnet dry run with a tiny epoch table; replay across two epochs;
      `task verify`.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/instructions/compression.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/api/src/bin/` (migration CLI), `DEV.md`

Dependencies: 1.4, 4.1.

Estimated scope: Large; required pre-rollout gate.

### Phase 5: Rollout, docs, and optional reclaim

#### Task 5.1: Devnet end-to-end matrix

Description: Exercise SOL and token settles, replay, collision, rotation, flag
off/on, and legacy marker reads in one devnet session. Record signatures.

Acceptance criteria:

- [ ] Every case above passes against real devnet RPC, not only the harness.
- [ ] Compute units and transaction sizes are captured for a compressed settle.

Verification:

- [ ] Signed-off devnet run appended to this plan.

Files likely touched: this plan, `DEV.md`

Dependencies: 3.3, 3.4, 4.2, 4.3.

Estimated scope: Medium.

#### Task 5.2: Staged production rollout

Description: Deploy in order: program + IDL (including the compatible SPL
program from Task 0.3), registry init, tree/bitmap/nullifier creation, API, web
client. Use `ReceiptMode`: shadow mode (compute proof, still write marker and
claim nullifier) → compressed for new invoices → default. Keep the legacy path
available for one full release cycle.

Acceptance criteria:

- [ ] A flag flip restores the old path without a program upgrade, and a
      rollback does not lose replay protection for invoices settled while the
      flag was on.
- [ ] No already-settled invoice can be paid again during either flag
      direction; all four transitions have a signed devnet test.
- [ ] `README.md` changes from "planned" to "live" and documents tree/bitmap/
      nullifier retention, the 900-slot rotation window, and the 3.900 SOL
      per-rotation rent with its operator/treasury split and the withdrawal
      floor that keeps the treasury's 2.666 SOL share in the pool.
- [ ] `app/utils/invoiceCost.ts` and its tests mirror the active on-chain path:
      legacy rent floor while the flag is off, 100/50 bps with no receipt rent
      once the compressed flag is on, while token invoice rent is still refunded.
- [ ] The `invoiceCost.test.ts` fixture uses live `Rent::minimum_balance(48)`
      (currently **1,224,960** lamports on mainnet; 894,080 on devnet) instead of
      a typed-in constant, and compressed cases assert `receiptRent = 0`,
      `recordFee = 0`, and `feeIsFloor = false`.

Verification:

- [ ] `task verify`, `task anchor:test`, smoke tests, devnet replay across both
      flag directions.

Files likely touched: `README.md`, `DEV.md`, `package.json`,
`apps/web/app/utils/invoiceCost.ts`, `apps/web/app/utils/invoiceCost.test.ts`,
`apps/web/app/composables/useFeeConfig.ts`

Dependencies: 5.1.

Estimated scope: Medium.

#### Task 5.3: Optional legacy marker reclaim (separate decision)

Description: Moving historical receipts into the trees would free their old
rent, but needs a destination policy and a batched migration. Explicitly
out of scope for the first release unless Task 0.1's decision covers it.

Acceptance criteria:

- [ ] A follow-up plan or an explicit "not doing this" note exists.
- [ ] No old marker is closed without its rent destination being decided.
- [ ] Every marker to be closed has its nullifier tag backfilled and verified
      before the close, so removing the marker fallback cannot re-open replay.

Verification: Review only.

Dependencies: 0.1, 5.2.

Estimated scope: Small.

## Checkpoints

- [ ] **After Phase 0:** fee replacement, replay design, CPI path, and README
      policy are written decisions; the mainnet-compatible program gate is
      either passed or the rollout is explicitly blocked.
- [ ] **After Checkpoint A:** registry, trees, receipt-mode flag, nullifier
      pages, occupancy bitmaps, and CPI work; `task verify` and
      `task anchor:test` pass.
- [ ] **After Checkpoint B:** settlement is correct on both paths and across all
      flag transitions; human review.
- [ ] **After Checkpoint C:** end-to-end payment works in checkout and Solana
      Pay without a receipt account, and legacy receipts still render.
- [ ] **Before completion:** `README.md` fee/rent sections switched from planned
      to live, `DEV.md` records retention/rotation, IDL regenerated
      (`pnpm anchor:idl`), plan moved to `plans/archive/`.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Anchor 1.2.1 vs SPL crate 0.29 incompatibility | High | Raw CPI to the deployed program; no crate, no downgrade; Task 0.2 confirms bytes on devnet |
| Mainnet SPL build lacks prepared-init | Critical | Task 0.3 is a hard gate: deploy/upgrade a compatible build and execute the full init sequence on mainnet before enabling the flag |
| Program-data hash was misread | Medium | Program account, programdata address, and programdata bytes are recorded separately; compatibility is proven by executing instructions, not hash equality |
| Authority transferred before tree init | High | Task 1.2 signs setup with the CLI authority, transfers last, then verifies PDA-signed CPI |
| Config flag needs realloc migration | Medium | Use a separate `ReceiptMode` PDA; no `CONFIG_SPACE`/`CONFIG_VERSION` bump or fixed-offset migration |
| Fee policy left undefined | Low | Resolved and recorded in `README.md`: 100/50 bps, no receipt-rent floor/flat/refund; token invoice rent still refunded; legacy path unchanged |
| Flag flip re-opens settled invoice | Critical | Both modes check marker + nullifier and claim nullifier atomically; all four transitions are acceptance-tested |
| Token treasury underfunded by removed refund | High | Keep `ensure_treasury_funded(payment_rent)` and the `payment_rent` refund on compressed token settles |
| API status misses compressed receipt | High | `fetch_payment`/`InvoiceReader::record` look up the compressed row before the marker decode; real `fetch_payment` tests |
| Solana Pay builder is stale | High | Task 3.4 rebuilds it from the current IDL with a parsed-wire fixture before compressed scope is complete |
| Hash collisions cause false `AlreadySettled` | High | Tagged nullifier pages: own tag fails, foreign tags are skipped; 8-byte tags inside an 8,192-slot page make a false positive a ~1e-10 birthday event |
| Collision probing adds proof/CU cost | Medium | Rotate at 60% fill; 128-probe cap; occupancy bitmaps and nullifier tags are byte reads, not Merkle proofs |
| Shared accounts cost more than they save at low volume | Medium | Epoch 0 is 8 pages / 2.67 SOL, not 683 SOL; float is ~6.6 SOL and the compressed path is only enabled at ~700k expected settlements (rent break-even is ~7.7k) |
| Legacy settle suddenly needs a nullifier page | Critical | Legacy path checks the marker only; a missing page never blocks it, so flag flips cannot cause an outage or a page-funding grief |
| Epoch growth rewrites tags while live | Medium | Copy per page, verify counts and a full scan, dual-check both epochs during the switch, keep the old epoch readable through the in-flight window |
| Stale proofs fail under bursts | Medium | Refresh on changelog updates; bounded retry with clear errors; 4 shards reduce contention |
| Rotation races with in-flight transactions | High | `current` + `previous` trees with matching occupancy generations and a 900-slot cutoff returning `RotationExpired` |
| Rotation rent unfunded (3.900 SOL per generation) | Low | `withdraw_treasury` reserves the next rotation's 2.666 SOL while compression is enabled, so the admin cannot withdraw it; step 2 is atomic and fails with `TreasuryUnderfunded` before creating anything, the balance alert covers settlement-refund drain, and the runbook verifies both the treasury's 2.666 SOL and the operator's 1.234 SOL + fees before step 1 |
| Nullifier table fills | Medium | Epoch expansion is a required pre-rollout gate with dual-check, verified tag copy, and ~70% alert |
| Indexer outage blocks new settlements | Medium | Keep legacy path flag; proof data is reconstructible from `ReceiptCommitted` + changelogs |
| Browser guesses a compressed leaf or receipt status | High | Rust API owns shard/leaf selection, collision probing and verification; browser only signs the plan |

## Rollout / Deployment

- [ ] **Program / migration:** pass Task 0.3 on the target cluster, upgrade the
      program and IDL, init registry and `ReceiptMode`, create tree authority,
      create and prepare four trees with the CLI authority before transfer,
      fund the treasury for the 2.666 SOL of generation-0 bitmap rent, create
      generation-0 bitmaps and initial nullifier pages, then flip the
      receipt-mode flag. From the flip on, Task 1.5's withdrawal floor keeps
      each later generation's 2.666 SOL in the pool.
- [ ] **API / data:** schema migration, proof service deploy, dual-path reader,
      rebuilt Solana Pay builder, then enable proof generation for new invoices.
- [ ] **Client:** quote reads `ReceiptMode`; verify the checkout shows 1% with no
      receipt rent once the compressed flag is on.
- [ ] **Smoke tests:** `pnpm smoke:silent-payment`, `pnpm smoke:guards`, plus
      compressed SOL/token settles and replay in both flag directions.
- [ ] **Rollback:** flip `ReceiptMode` back to marker mode; previously settled
      invoices remain protected by their nullifier, trees/bitmaps/pages remain
      funded, and no receipt is lost.

## Definition Of Done

- [ ] All acceptance criteria are met.
- [ ] `task verify` and `task anchor:test` pass.
- [ ] `README.md` describes the live fee basis and retention policy; `DEV.md`
      records tree addresses, authority, nullifier epoch derivation, the 900-slot
      rotation cutoff, and rollback.
- [ ] The mainnet-compatible compression program gate has passed on every
      cluster where the flag is enabled.
- [ ] The plan is moved to `plans/archive/` and listed in `plans/README.md`.

## Open Questions

- Is reclaiming legacy markers worth a batched migration, or should their rent
  be left as a sunk cost? (The compressed path has no receipt-rent refund, but
  old markers still hold rent whose destination would need a decision, and their
  tags must be backfilled before any close.)
- Should `verifyLeaf` be exposed as a read-only CPI for trustless receipt checks,
  or is off-chain verification against `ReceiptCommitted` sufficient?
- Should nullifier tags shrink from 8 to 6 bytes? A 6-byte tag cuts a page from
  65,544 to 49,160 bytes, so epoch 0 falls from 2.67 SOL to 2.00 SOL and a
  receipt from ~40,700 to ~30,560 lamports. It is safe because false positives
  depend only on per-page occupancy (8,192 slots), where 6 bytes give a
  ~`1e-10` birthday risk. A 4-byte tag is **rejected**: ~2% of full pages would
  contain a colliding pair, which permanently blocks a valid invoice. Decision
  needed before Task 1.4, since the slot width is fixed on chain.
- Should the four occupancy bitmaps (2.67 SOL per generation) be dropped by
  folding the leaf index into the nullifier slot? That saves the rent but
  couples replay ordering to tree-slot allocation and removes the witness that
  makes "first unset bit" checkable without a proof. Deferred; revisit only if
  bitmap rent becomes a real line item.
- None blocking. The mainnet SPL version question is now a Task 0.3 gate, not an
  open question: compatibility is proven by executing the required instruction
  set on the target cluster.
