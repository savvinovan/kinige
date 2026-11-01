# ADR-0003: Signing, notarization and updater keys

Date: 2026-10-19. Status: proposed (slice 0), awaiting the user's confirmation of the "where they live" part — this is a decision about the user's own secrets.
Sources: PRD §12.3, `docs/plan/research/tauri-stack.md`, slice 0 decisions (item 2).

## Context
In slice 0 the `.app` is built and signed locally ad hoc; there is no macOS runner and no signing secrets in CI.
Developer ID, notarization and the updater are slice 10. The decision is needed now, so that by slice 10 the key does not turn out to be lying just anywhere.

## Decision
| Key                                                                       | Appears  | Where it lives                                                                                                                 | Who has access    |
|---------------------------------------------------------------------------|----------|--------------------------------------------------------------------------------------------------------------------------------|-------------------|
| Ad-hoc signature (`signingIdentity: "-"`)                                 | slice 0  | there is no key                                                                                                                | any local build   |
| Developer ID Application certificate (+ private key)                      | slice 10 | Keychain on the owner's Mac; a password-protected `.p12` backup in the owner's password manager                                | the project owner |
| Notarization credentials (Apple ID / app-specific password or an API key) | slice 10 | the owner's password manager; passed to the build via process environment variables, not via a file                            | the owner         |
| Updater private key (`tauri signer generate`)                             | slice 10 | the owner's password manager + an offline copy; passed to the build via `TAURI_SIGNING_PRIVATE_KEY` in the process environment | the owner         |
| Updater public key                                                        | slice 10 | `tauri.conf.json` (public)                                                                                                     | everyone          |

Rules:
- Secrets do not go into the repository, `.env`, logs or Docker images. According to `research/tauri-stack.md`, the updater key is not read from `.env` — only from the process environment.
- While the build is local, CI (GitHub Actions) gets no signing secrets: a macOS runner already exists, but only for window screenshots (`screens.yml`) and macOS code checks (`ci.yml`), without signing. When a signed build appears (not earlier than slice 10), the signing secrets go into GitHub Actions secrets only for the protected release branch/tags.
- Update signing is mandatory and cannot be turned off (Tauri updater), so losing the updater private key means being unable to release an update for installed copies.

## Revocation and compromise
- Developer ID: revoke the certificate in the Apple Developer account, issue a new one, rebuild and re-sign the releases. How exactly Gatekeeper treats old builds that were already notarized is not verified; to be clarified in slice 10.
- Updater key: installed apps trust only the public key built into them. On compromise, a release is needed that is signed with the **old** key and contains the **new** public key, after which the old key is destroyed. If the old key is lost, only a manual reinstall remains. Not verified in practice: verify in slice 10 on a test channel before the first release.

## Not settled
Whether to use a separate updater key for the test channel is decided in slice 10.
