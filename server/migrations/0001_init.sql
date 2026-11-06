-- Kinige server, schema 1 (spike S4c-1, ADR-0019). Everything the server keeps is here; nothing in it is plain text of the user's
-- journal: the only readable values are the login, ids, HLCs and sizes the protocol shows the server anyway (ADR-0001 §10).

-- The generations of this installation, oldest first, the current one last (ADR-0001 §6.3). One row until a restore adds one (S4c-2).
-- The first is a random 63-bit number; it is opaque.
CREATE TABLE lineage (
    pos        integer PRIMARY KEY,
    generation bigint  NOT NULL UNIQUE
);
INSERT INTO lineage (pos, generation) VALUES (1, (random() * 9.0e18)::bigint + 1);

CREATE TABLE accounts (
    id           text     PRIMARY KEY,
    login        text     NOT NULL UNIQUE,
    salt         bytea    NOT NULL,
    profile      smallint NOT NULL,
    -- crypto_generichash of the auth_key and of the rc_auth_key (ADR-0001 §8.3). Never the keys.
    auth_hash    bytea    NOT NULL,
    rc_hash      bytea    NOT NULL,
    key_id       integer  NOT NULL,
    wrap_pw      bytea    NOT NULL,
    wrap_rc      bytea    NOT NULL,
    -- Bumped by every password change and recovery; a password change swaps only on the version it was derived against (§8.5).
    cred_version bigint   NOT NULL DEFAULT 1,
    -- The account lock is this row: `seq` is issued under `SELECT ... FOR UPDATE` of it, in the transaction of the write (§10.2).
    seq_head     bigint   NOT NULL DEFAULT 0,
    -- The counter of compactions that dropped a `purged` (§6.3).
    epoch        bigint   NOT NULL DEFAULT 0,
    created_ms   bigint   NOT NULL
);

-- The hash of a session token (32 random bytes); the token itself is never stored (§8.4).
CREATE TABLE sessions (
    token_hash bytea  PRIMARY KEY,
    account_id text   NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_ms bigint NOT NULL
);
CREATE INDEX sessions_account ON sessions (account_id);

-- The server log of one account: the open header's routing fields and the envelope bytes as they came (§5.2, §A.1).
CREATE TABLE envelopes (
    account_id text    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    seq        bigint  NOT NULL,
    op_id      text    NOT NULL,
    entity     text    NOT NULL,
    entity_id  text    NOT NULL,
    kind       text    NOT NULL,
    -- {"<group>": "<HLC>"}; empty for a purged.
    groups     jsonb   NOT NULL,
    at_ms      bigint  NOT NULL,
    len        integer NOT NULL,
    bytes      bytea   NOT NULL,
    PRIMARY KEY (account_id, seq)
);
CREATE INDEX envelopes_entity ON envelopes (account_id, entity, entity_id);

-- Every op_id ever accepted and the hash of its bytes, kept after compaction (idempotency, §A.4 step 3).
CREATE TABLE ops (
    account_id text   NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    op_id      text   NOT NULL,
    seq        bigint NOT NULL,
    bytes_hash bytea  NOT NULL,
    PRIMARY KEY (account_id, op_id)
);

-- The maximum HLC per group among an entity's accepted headers: the index of `stale_purge` (§3.5). HLCs compare as bytes.
CREATE TABLE entity_known (
    account_id text NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    entity     text NOT NULL,
    entity_id  text NOT NULL,
    grp        text NOT NULL,
    hlc        text NOT NULL COLLATE "C",
    PRIMARY KEY (account_id, entity, entity_id, grp)
);

-- Entities whose `purged` was accepted, with the accepted snapshot (§3.5, ADR-0017).
CREATE TABLE purged_entities (
    account_id text   NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    entity     text   NOT NULL,
    entity_id  text   NOT NULL,
    seq        bigint NOT NULL,
    purged_of  jsonb  NOT NULL,
    PRIMARY KEY (account_id, entity, entity_id)
);

-- Compactions that dropped a `purged`, each with the generation it was made in and its time (§6.3, ADR-0017).
CREATE TABLE drops (
    account_id   text   NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    generation   bigint NOT NULL,
    epoch        bigint NOT NULL,
    seq          bigint NOT NULL,
    compacted_ms bigint NOT NULL,
    PRIMARY KEY (account_id, generation, epoch)
);
