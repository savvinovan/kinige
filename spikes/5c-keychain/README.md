# Spike 5c: the keychain across two ad-hoc-signed builds

Slice 5c (KINIG-151, ADR-0021) keeps the Telegram bot token in the OS secret store. On macOS that is a generic-password item. The question
this crate answers: does a second ad-hoc-signed build read the item that the first build wrote?

Result (CI runs 44747551144, 44749318159 and 44749995262, macos-15; details and the ruling in ADR-0021): no, not silently. The second build
blocks on the keychain prompt (headless: the 20 s alarm, status 142). A write by the second build succeeds without a prompt and moves the
item's ownership to it. An item made with `security add-generic-password -A` does not lift the prompt. Not verified: the prompt on a real
desktop, "Always Allow" across a real update, and a stable signing identity (the self-signed attempt in `run.sh` section 3 never signed:
`codesign` found no identity, so both builds stayed ad hoc and that section proves nothing).

## Run it again

The manual CI job was removed before the PR. To repeat the check on a macOS runner, put a job like this in a throwaway workflow on a
branch and run it with `gh workflow run`:

```sh
cd spikes/5c-keychain
for tag in a b; do
  BUILD_TAG=$tag CARGO_TARGET_DIR="$RUNNER_TEMP/kc-$tag" cargo build --release
  cp "$RUNNER_TEMP/kc-$tag/release/keychain-check" "$RUNNER_TEMP/kc-$tag.bin"
  codesign --force --sign - "$RUNNER_TEMP/kc-$tag.bin"
done
bash run.sh "$RUNNER_TEMP/kc-a.bin" "$RUNNER_TEMP/kc-b.bin"
```

`run.sh` prints every result and never stops on an error. The program handles a fixed fake secret only. Nothing here runs on a
developer's Mac.
