-- Kinige server, schema 3 (spike S4c-2, round 1 of the review, ADR-0020): the credential journal.
--
-- Every change of an account's credentials (a password change, a recovery, new words) writes the credentials the account has after it, with
-- the time, in the transaction of the change. A restore re-applies the newest entry at or after the backup's cut time to each restored
-- account, so that a password changed after a backup does not come back as the old one (ADR-0001 §8.9). Nothing here is more than
-- `accounts` already holds; the account is named by crypto_generichash of its id, like the deletion journal, and its rows go with it.
CREATE TABLE credential_changes (
    account_hash bytea    NOT NULL,
    cred_version bigint   NOT NULL,
    salt         bytea    NOT NULL,
    profile      smallint NOT NULL,
    auth_hash    bytea    NOT NULL,
    rc_hash      bytea    NOT NULL,
    key_id       integer  NOT NULL,
    wrap_pw      bytea    NOT NULL,
    wrap_rc      bytea    NOT NULL,
    changed_ms   bigint   NOT NULL,
    -- New words keep the credential version, so the time is part of the key.
    PRIMARY KEY (account_hash, cred_version, changed_ms)
);
