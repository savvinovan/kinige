# KINIG-99: spike S4c-2, blobs, backup and restore, docker compose

**Budget:** about 10 h to merge (code 5–6 h, guardian and review 4 h). Stop and report at 20 h.

**Task:** Huly KINIG-99, under KINIG-20 (slice 11). Branch `spike/S4c2-blobs-backup`. The PR title and the first commit start with `KINIG-99: …`. No release: nothing changes in the app.

**Sources:**
- PRD §10.1, §10.3 (account deletion) and §14. The S4c criterion: restore from a backup → clients do a full resync. A restore into a clean installation is mandatory.
- ADR-0001 §6.3, §7, §8.9 and appendix §A.2. ADR-0017's branch-cut rule. ADR-0019 "S4c-2".
- User decisions of 06.11.2026:
  - **Q14 = 30 days:** server backups are kept for 30 days and then rotated.
  - **Q9:** blobs on disk **and** on an S3-compatible store, both supported.

S4c-1 built the server core. **S4c-2 asks: can the server keep its promises about files and about restore?**
- A blob is never lost while any backup still refers to it.
- A restore, in place or into a clean installation, makes every client resync without losing or resurrecting anything.
- A self-hoster can run it all with one `docker compose up`.

Where a rule cannot be built as written, fix it and edit ADR-0001 in place, marked "S4c, <date>", in the same PR.

## Scenario
1. `cargo test -p kinige-server --features pg-tests` also runs blobs against both backends, backup and restore, and account deletion.
   - The disk backend uses a temporary directory.
   - The S3 backend uses a throwaway S3-compatible container. CI starts it as a service; locally it runs on ubuntu-server.
2. The S4a simulation with `real-server` now also runs **property 8, the restore mixes and the directed restore scenarios** against the real server. Each restore is a real dump of the server, restored into a clean database, through the same code path the operator uses.
3. In CI, `docker compose up` on a runner brings up the server, Postgres and a blob volume from the repo's `deploy/` directory. A smoke script then:
   - registers, pushes, uploads a blob and pulls;
   - takes a backup;
   - restores it into a fresh stack;
   - checks that a pull with the old cursor answers `GenerationChanged`, and that the blob downloads byte for byte.

## Included
- **Blobs** (§7, §A.2), keyed by `(account_id, remote_blob_id)`.
  - **Resumable upload by offset.**
    - An unfinished upload can be overwritten.
    - Publishing checks only the declared size and the §A.2 framing that the server can see (24 + chunks of `1 MiB + 17`). After that the blob is immutable.
    - Publishing the same id again answers "already exists", which counts as success.
  - **Download** with a range.
  - **Delete a blob of one's own account**, for the re-upload after `missing`.
  - **How a purge of an attachment reaches the blob:** `remote_blob_id` sits in the encrypted `core` group, so the server cannot read it from a `purged`. Follow ADR-0001 if it says how. If it does not, decide (for example, the purging device sends the blob delete itself), write it into §7 and test it.
  - Limits come before any allocation: 2 GiB per file (PRD §4.2), and a per-account quota as a config value.
  - Every blob request is authenticated first, as push is.
  - **Storage:** one small interface with two backends.
    - **Disk:** write to a temporary file, `fsync`, then rename.
    - **S3-compatible.**
    - The crate is your choice. `object_store` covers both; check its licence and maturity first.
  - **Pick the S3 test target after checking the facts.** The PRD says MinIO has been in maintenance mode since late 2025. Verify that, compare Garage and SeaweedFS, run the tests against one of them, and record the choice and the reasons in ADR-0020.
- **Delayed physical deletion** (§7, PRD §10.1). A deleted blob is only marked deleted. The object is removed once it is older than the backup retention, 30 days plus one backup interval, both config values. So every backup that can still be restored finds its blobs.
  - A restore drops objects that no restored row refers to, through the same delayed path.
  - Orphans from uploads that were never published are cleaned after a config age.
- **Backup.** `kinige-server backup <dir>` writes:
  - a consistent snapshot of Postgres: `pg_dump` or the server's own dump, your choice, documented;
  - the blobs;
  - a manifest with the cut time, the format version and the hashes.

  Consistency rests on two facts: blobs are immutable after publication, and their deletion is delayed beyond retention. Write that argument down in ADR-0020 and test it: a blob published or deleted during the backup must not break the restore.
  - **Rotation:** a `kinige-server backup --prune` (or a step of the backup) removes backups older than 30 days, as a config value.
  - The PRD §10.3 wording now reads "30 days".
- **Restore.** `kinige-server restore <dir>`, in place or into a clean installation, through one code path. It loads the dump, appends a fresh generation to the lineage, records the generation's **cut time** (the backup's time) and copies the blobs.
  - **The branch-cut rule of ADR-0017** is built in both places:
    - **the real server:** each drop stores its compaction time (already there); `GenerationChanged` returns the cut time of the generation it names; the pull and push of a cursor whose generation is outside the lineage send it back, and count a drop as missed when it was compacted after that cut time;
    - **the S4a in-memory server**, so that the two models stay one rule.

    The `review_round_3_*` scenarios in `spikes/S4a-sync/tests/scenarios.rs` change from "pins today's limitation" to "passes".
  - **Account deletion** (§8.9):
    - The server deletes the account, its wraps, hashes, sessions and envelopes. Its blobs get the delayed deletion above.
    - **The deleted account must not come back with an old backup.** Keep a deletion journal of `generichash(account_id)` and the time. Restore re-applies every deletion from the journal that is newer than the backup's cut time. The journal comes from the current installation when there is one, otherwise from the newest backup.
    - Write the rule in §8.9 and test it: delete, restore an older backup, and the account is gone.
- **The S4c-1 carry-overs:**
  - a push `stale` test with a generation that is in the lineage but not current;
  - a read timeout on request bodies (push and blob upload): a client that sends a byte a minute is cut off;
  - the reverse proxy's `client_max_body_size` matched to the server's caps, in the compose example.
- **`deploy/`:**
  - a multi-stage `Dockerfile`: a non-root user, a pinned base image by digest, and the server stopping on SIGTERM;
  - `docker-compose.yml`: the server, `postgres:17` pinned, a volume for blobs, an optional S3 through environment variables, a healthcheck, and a reverse proxy with TLS. Caddy or another, your choice; the TLS setup is documented and not tested beyond startup;
  - a `.env.example` with no real secret;
  - a backup cron example.
  - **CI:** a `compose` job on `ubuntu-24.04` runs the smoke script, with its image pinned.
  - **Locally:** use the remote Docker on ubuntu-server, with the compose project name `kinige-s4c2-<something>` and the containers and volumes removed after.
- **Tests (rule 12):**
  - garbage on every new endpoint, the restore and backup commands included: truncated manifests, missing files, a wrong hash;
  - the plain-text scan from S4c-1 now covers the blob store and the backup directory.
- **Docs:**
  - `server/README.md`: blobs, backup, restore, deletion and self-hosting;
  - ADR-0020 "S4c-2 results", half a page;
  - ADR-0001 edits marked "S4c", with §A.6 extended for blobs;
  - a PRD changelog line, with the S4c row and Q9/Q14 marked closed;
  - the CLAUDE.md lines for `deploy/`.

## Not included
- Deploying to the homelab or the k3s cluster, the homelab deployer, any change on ubuntu-server beyond throwaway `kinige-` containers. Exposing the server to the internet (the rest of Q9, Cloudflare): slice 11, with the user.
- Any client code, UI or app change, and the client's streaming or resumable upload: slice 11.
- Crypto-shredding of backups (P2), DEK rotation, metrics, real rate-limit numbers.
- Changes to `kinige-store`, the app schema, or `kinige-crypto`'s formats.

## What "done" looks like
- CI is green on all jobs, including `server`, with S3 and with S4a `real-server` running property 8 and the restore mixes, and the new `compose` job.
- The README maps each of these to the test that covers it:
  - blob upload, resume, publish, "already exists", download, delete, purge;
  - delayed deletion across a restore;
  - backup during writes;
  - restore in place and into a clean installation;
  - the branch-cut rule;
  - account deletion with an old backup.
- ADR-0020 records the S3 choice with its facts and the consistency argument.
- The local containers and volumes on ubuntu-server are removed: `docker ps -a --filter name=kinige-` and `docker volume ls --filter name=kinige-` show nothing of this spike.

## Touches sensitive areas
Yes: the sync protocol (restore, generations, the stale rules), account deletion, and what the server and its backups store.
- `sync-crypto-guardian` reviews before merge. It checks the restore and the branch-cut rule against §6.3 and ADR-0017, the deletion against §8.9 and PRD §10.3, and that backups and the blob store hold no plain text.
- `slice-reviewer` checks the spec, the evidence and the CI.
- New dependencies go through `cargo-deny`: the storage crate and anything it pulls. So do the new images: Postgres, the S3 server and the proxy, all pinned by digest.
