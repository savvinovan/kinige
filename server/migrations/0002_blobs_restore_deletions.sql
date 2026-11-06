-- Kinige server, schema 2 (spike S4c-2, ADR-0020): blobs, the cut time of a restored generation, and the deletion journal.

-- The time of the backup a generation was cut from (ADR-0017, ADR-0001 §6.3): 0 for a generation no restore made. A device on a generation
-- that a later restore abandoned sends it back, and the server counts a drop as missed when it was compacted at or after it.
ALTER TABLE lineage ADD COLUMN cut_ms bigint NOT NULL DEFAULT 0;

-- One row per blob that was ever published (ADR-0001 §7, §A.2). The bytes are in the object store under `<account_id>/<blob_id>`; an
-- unfinished upload is a file in the staging directory and has no row. `deleted_ms` marks a blob deleted by its device, by a purge of
-- its attachment or by the deletion of its account: the object is removed only after the backup retention (ADR-0020). There is no
-- foreign key to `accounts`: the row of a deleted account stays until the object is gone.
CREATE TABLE blobs (
    account_id   text   NOT NULL,
    blob_id      text   NOT NULL,
    size         bigint NOT NULL,
    published_ms bigint NOT NULL,
    deleted_ms   bigint,
    PRIMARY KEY (account_id, blob_id)
);
CREATE INDEX blobs_deleted ON blobs (deleted_ms) WHERE deleted_ms IS NOT NULL;

-- The deletion journal (ADR-0001 §8.9): crypto_generichash of the account id and the time of the deletion, nothing else. A restore
-- applies the entries newer than the backup's cut time, so a deleted account does not come back with an old backup.
CREATE TABLE deletions (
    account_hash bytea  PRIMARY KEY,
    deleted_ms   bigint NOT NULL
);
