//! The schema every database is brought up to: the tables and indexes a fresh
//! database is created with, each statement idempotent.

pub(super) const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS users (
        id            TEXT PRIMARY KEY,
        email         TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        wallet        TEXT,
        created_at    INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sessions (
        token_hash TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS sessions_user_id_idx ON sessions(user_id)",
    "CREATE INDEX IF NOT EXISTS sessions_expires_at_idx ON sessions(expires_at)",
    "CREATE TABLE IF NOT EXISTS discount_codes (
        id          TEXT PRIMARY KEY,
        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        code        TEXT NOT NULL,
        percent_off INTEGER NOT NULL CHECK (percent_off BETWEEN 1 AND 99),
        created_at  INTEGER NOT NULL,
        UNIQUE (user_id, code)
    )",
    "CREATE INDEX IF NOT EXISTS discount_codes_user_id_idx ON discount_codes(user_id)",
    // A code is once per buyer, and the buyer is a wallet: a payment link can be
    // opened without an account, so an account id would leave most redemptions
    // uncounted. The invoice is kept because it is the evidence — `use_code`
    // reads the payer off the settled invoice rather than trusting a request.
    "CREATE TABLE IF NOT EXISTS discount_redemptions (
        id         TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        code       TEXT NOT NULL,
        buyer      TEXT NOT NULL,
        invoice    TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (user_id, code, buyer)
    )",
    "CREATE INDEX IF NOT EXISTS discount_redemptions_user_id_idx ON discount_redemptions(user_id)",
    // A saved payment keeps a copy of the code that already *is* the invoice:
    // the API does not know invoices, only what the account wants to see again.
    // `shop_id` is the shop the seller filed it under, when they picked one.
    "CREATE TABLE IF NOT EXISTS saved_payments (
        id         TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        payment_id TEXT NOT NULL,
        payload    TEXT NOT NULL,
        shop_id    TEXT,
        created_at INTEGER NOT NULL,
        UNIQUE (user_id, payment_id)
    )",
    "CREATE INDEX IF NOT EXISTS saved_payments_user_id_idx ON saved_payments(user_id)",
    // A seller may keep several shops — a café and a bakery — so a shop is a
    // row of its own with an id, listed on the profile and opened on its own
    // page.
    "CREATE TABLE IF NOT EXISTS shops (
        id          TEXT PRIMARY KEY,
        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        currency    TEXT NOT NULL DEFAULT 'sol',
        created_at  INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS shops_user_id_idx ON shops(user_id)",
    // Products belong to a shop, and keep the account id as well: every read
    // is scoped to the signed-in account, so one shop's id in a request can
    // never reach another account's catalogue. `price_lamports` is the
    // canonical price — the receipt in every invoice is built from it.
    "CREATE TABLE IF NOT EXISTS products (
        id             TEXT PRIMARY KEY,
        user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        shop_id        TEXT NOT NULL,
        name           TEXT NOT NULL,
        details        TEXT NOT NULL DEFAULT '',
        category       TEXT NOT NULL DEFAULT '',
        price_lamports INTEGER NOT NULL,
        created_at     INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS products_user_id_idx ON products(user_id)",
    // The headings a shop has used, kept per shop so the editor can suggest them
    // the next time rather than asking the seller to remember their own menu.
    "CREATE TABLE IF NOT EXISTS shop_categories (
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        shop_id    TEXT NOT NULL,
        name       TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (shop_id, name)
    )",
    "CREATE TABLE IF NOT EXISTS password_resets (
        token_hash TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    // One live sign-in/link challenge per address, single use, short lived.
    "CREATE TABLE IF NOT EXISTS wallet_challenges (
        address    TEXT PRIMARY KEY,
        purpose    TEXT NOT NULL,
        nonce      TEXT NOT NULL,
        message    TEXT NOT NULL,
        issued_at  INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    // A settled invoice's facts, copied off its settle transaction once. The
    // chain keeps the only authoritative record — the marker — and this is a
    // cache: losing every row costs one history read, never correctness, which
    // is why nothing here is a foreign key or a source of truth.
    "CREATE TABLE IF NOT EXISTS settled_payments (\n        invoice    TEXT NOT NULL,\n        endpoint   TEXT NOT NULL,\n        seller     TEXT NOT NULL,\n        reference  TEXT NOT NULL,\n        mint       TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        memo       TEXT NOT NULL,\n        payer      TEXT NOT NULL,\n        created_at INTEGER NOT NULL,\n        settled_at INTEGER NOT NULL,\n        PRIMARY KEY (invoice, endpoint)\n    )",
    // A wallet belongs to at most one account.
    "CREATE UNIQUE INDEX IF NOT EXISTS users_wallet_unique_idx \
     ON users(wallet) WHERE wallet IS NOT NULL",
    // Sellers who want to take real money, waiting for a live cluster. The
    // email is the key, so one address joins once; nothing here is linked to
    // an account, because someone on the list may not have one.
    "CREATE TABLE IF NOT EXISTS waitlist (\n        email      TEXT PRIMARY KEY,\n        created_at INTEGER NOT NULL\n    )",
    "CREATE INDEX IF NOT EXISTS password_resets_user_id_idx ON password_resets(user_id)",
    // The shops subscription: one row per paid period. `payment_id` is the
    // settled invoice address, so the same payment cannot buy two periods.
    "CREATE TABLE IF NOT EXISTS subscriptions (\n        id         TEXT PRIMARY KEY,\n        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        currency   TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        payment_id TEXT NOT NULL,\n        paid_at    INTEGER NOT NULL,\n        expires_at INTEGER NOT NULL,\n        UNIQUE (user_id, payment_id)\n    )",
    "CREATE INDEX IF NOT EXISTS subscriptions_user_id_idx ON subscriptions(user_id)",
    // The invoice a subscription was quoted at, kept until it settles: the
    // amount the buyer agreed to is the amount `confirm` requires on chain,
    // whatever the rate has done since.
    "CREATE TABLE IF NOT EXISTS subscription_invoices (\n        payment_id TEXT PRIMARY KEY,\n        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        currency   TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        created_at INTEGER NOT NULL\n    )",
    // Every settlement the relayer paid the network fee for. The caps in
    // `sponsor.rs` read this table, and a row is written only after the cluster
    // accepted the transaction, so the count is spend rather than attempts. The
    // signature is the id: one sponsored settle cannot be counted twice.
    //
    // The wallet is the key the caps count, not the account: a buyer who scanned
    // a printed code has no account here, and the cap has to hold for them too.
    // `user_id` is what we happen to know about them, never a requirement.
    "CREATE TABLE IF NOT EXISTS sponsored_settlements (\n        id         TEXT PRIMARY KEY,\n        wallet     TEXT NOT NULL,\n        user_id    TEXT,\n        cluster    TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        created_at INTEGER NOT NULL\n    )",
    // A browser's push subscription: one row per device, keyed by the endpoint
    // the push service gave it. `locale` is the copy the reminder is written
    // in, because a notification arrives outside the app where no i18n runs.
    "CREATE TABLE IF NOT EXISTS push_subscriptions (\n        endpoint   TEXT PRIMARY KEY,\n        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        p256dh     TEXT NOT NULL,\n        auth       TEXT NOT NULL,\n        locale     TEXT NOT NULL DEFAULT 'en',\n        created_at INTEGER NOT NULL\n    )",
    "CREATE INDEX IF NOT EXISTS push_subscriptions_user_id_idx ON push_subscriptions(user_id)",
    // One reminder per account per month: the cron runs daily, and this is what
    // stops a month of mornings from becoming a month of notifications.
    // One starter-token claim per account. The row is reserved before the
    // transfer is sent, so a double tap cannot send twice; `signature` stays
    // empty until the cluster accepted it, and a failed send deletes the row.
    "CREATE TABLE IF NOT EXISTS faucet_claims (\n        user_id         TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,\n        wallet          TEXT NOT NULL,\n        cluster         TEXT NOT NULL,\n        sol_lamports    INTEGER NOT NULL,\n        token_base_units INTEGER NOT NULL,\n        signature       TEXT NOT NULL,\n        claimed_at      INTEGER NOT NULL\n    )",
    "CREATE TABLE IF NOT EXISTS push_nudges (\n        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        month_index INTEGER NOT NULL,\n        sent_at     INTEGER NOT NULL,\n        PRIMARY KEY (user_id, month_index)\n    )",
    // What an invoice is called and what it is for: the chain stores neither,
    // so the seller's own words are kept here when the invoice is made, and the
    // history can show a subject instead of an amount with no story.
    // The invoices this API made, one row per address the invoice can be paid
    // at. It began as the seller's own copy of a name and description — the
    // chain keeps neither — and the feed lists from it now: `amount` and
    // `reference` are what a status read needs, so the history asks the cluster
    // whether an invoice settled rather than for every payment ever made.
    "CREATE TABLE IF NOT EXISTS invoice_meta (\n        id          TEXT PRIMARY KEY,\n        seller      TEXT NOT NULL,\n        reference   TEXT NOT NULL DEFAULT '',\n        cluster     TEXT NOT NULL DEFAULT '',\n        endpoint    TEXT NOT NULL DEFAULT '',\n        currency    TEXT NOT NULL DEFAULT '',\n        mint        TEXT NOT NULL DEFAULT '',\n        amount      INTEGER NOT NULL DEFAULT 0,\n        memo        TEXT NOT NULL DEFAULT '',\n        seller_user TEXT,\n        reusable_id TEXT NOT NULL DEFAULT '',\n        paid_at     INTEGER NOT NULL DEFAULT 0,\n        name        TEXT NOT NULL,\n        description TEXT NOT NULL,\n        items       TEXT NOT NULL DEFAULT '',\n        created_at  INTEGER NOT NULL\n    )",
    // A reusable code: one offer — price, currency, title, words — that any
    // visitor pays by minting a fresh one-time invoice from it. The chain can
    // settle an invoice once, so the repeatable thing is the row, not an
    // address. The seller is the account that made it; `seller_user` is what
    // ties every payment it mints to that account's history.
    "CREATE TABLE IF NOT EXISTS reusable_links (\n        id          TEXT PRIMARY KEY,\n        seller      TEXT NOT NULL,\n        seller_user TEXT NOT NULL,\n        shop_id     TEXT,\n        cluster     TEXT NOT NULL,\n        endpoint    TEXT NOT NULL DEFAULT '',\n        currency    TEXT NOT NULL,\n        amount      INTEGER NOT NULL,\n        memo        TEXT NOT NULL DEFAULT '',\n        name        TEXT NOT NULL DEFAULT '',\n        description TEXT NOT NULL DEFAULT '',\n        created_at  INTEGER NOT NULL\n    )",
    "CREATE INDEX IF NOT EXISTS reusable_links_seller_idx ON reusable_links(seller_user, created_at)",
    // Which wallet a session key spends for. A payment made from an allowance is
    // signed by that key, not by the buyer's wallet, so the settle leaves a payer
    // the feed would not otherwise recognise as this buyer. The chain keeps the
    // pair — a budget account holds the buyer and its session key — and this is
    // what one look at it learned, so a later load matches the payment to the
    // wallet with a row read instead of another scan.
    "CREATE TABLE IF NOT EXISTS session_keys (\n        session_key TEXT NOT NULL,\n        wallet      TEXT NOT NULL,\n        endpoint    TEXT NOT NULL,\n        cluster     TEXT NOT NULL,\n        first_seen  INTEGER NOT NULL,\n        PRIMARY KEY (session_key, endpoint, cluster)\n    )",
    "CREATE INDEX IF NOT EXISTS session_keys_wallet_idx ON session_keys(wallet, endpoint, cluster)",
    // When this wallet's allowances were last looked up on chain. A wallet with
    // none would otherwise ask the cluster to confirm that on every feed load.
    "CREATE TABLE IF NOT EXISTS allowance_checks (\n        wallet     TEXT NOT NULL,\n        endpoint   TEXT NOT NULL,\n        cluster    TEXT NOT NULL,\n        checked_at INTEGER NOT NULL,\n        PRIMARY KEY (wallet, endpoint, cluster)\n    )",
    // An invoice row written before rows carried an amount is filled from the
    // chain once, the first time its seller opens the feed. This records the ids
    // the chain had nothing to say about, so a dead testnet invoice is not asked
    // about on every load afterwards.
    "CREATE TABLE IF NOT EXISTS invoice_backfill (\n        id       TEXT PRIMARY KEY,\n        endpoint TEXT NOT NULL,\n        tried_at INTEGER NOT NULL\n    )",
];
