# Slice 7a: Voice notes

**Budget:** ~4 h of code, ~8 h until ready for acceptance; if not ready after 16 h, stop and report (§12.1a). Order: the probe on both runners, the store command and the protocol route, the recorder, the player, summaries, screenshots. **Depends on:** 7. Huly **KINIG-16**; branch `slice/7a-voice-notes`, the PR and its first commit are titled `KINIG-16: …`. **Sources:** PRD §2.1, §5, §12.3, §15; ADR-0014; ADR-0001 appendix §C.6; ADR-0011; slice 7 decisions 4–6.

## Scenario
1. In Inbox the field is empty. I press the microphone on the right. The first time, macOS asks "Kinige would like to access the microphone"; I allow it. The bar turns into the recording bar: a red dot, the timer `0:01`, a level meter, a trash button on the left and the round accent Send on the right. The bar keeps its height.
2. I talk for 40 seconds and press Send (or Enter). The entry appears in the feed at once: a play button, a seek bar, `0:40` and `1×`. It has no text.
3. I press play. The time runs `0:03 / 0:40`. `1×` switches to `1.5×` and then `2×`. Starting another voice note pauses the first one.
4. I start a recording and press the trash button or Escape. Under 5 seconds it is dropped at once. Longer than that, the bar asks "Discard the recording?" with "Discard" and "Keep recording", and the recording goes on while it asks.
5. I unplug the headset mid-recording. The bar says "Microphone disconnected · 0:12" and offers Send and Discard. Nothing is lost silently.
6. Access is off in System Settings: the line above the field says "Microphone access is off" with "Open settings". No microphone: "No microphone found". A busy device: "The microphone is unavailable".
7. I `kill -9` the app and start it again. The sent note is there and plays. A recording that was still running is gone, as in Telegram.
8. `~/Documents/Kinige/Inbox/2026/2026-11-04.md` has `[voice-2026-11-04-213012.m4a](<2026-11-04/voice-2026-11-04-213012.m4a>) (0:40)`, and the file is in the day folder. Search for "voice" finds it. The reply quote, "On this day" and the capture context say "Voice note · 0:40". The forest card says "5 notes · 1 voice note".
9. My Telegram voice messages (imported by slice 7 as `audio` file cards) now show the same player. A format this webview can't play shows "Can't play here" with "Open in default app".

## Included
- **Probe first (≤ 1 h, a `7a-probe` scenario on both runners).** Record in `docs/screens/7a/formats.txt`: `isSecureContext` and `navigator.mediaDevices` on `tauri://localhost` (WKWebView) and `http://tauri.localhost` (WebView2); `MediaRecorder.isTypeSupported` for `audio/mp4;codecs=mp4a.40.2`, `audio/mp4` and `audio/webm;codecs=opus`; `canPlayType` for those plus `audio/ogg; codecs=opus`; and whether the fake source below produces a non-empty file on that runner; the observed bytes per second of a 5 s recording (does the engine honour `audioBitsPerSecond`?); and whether setting `audio.currentTime = 20` on a recorded file served over a `Range` route lands near 20 s (a fragmented MP4 may not seek). **If `mediaDevices` is missing in WKWebView, stop and report:** the native fallback would be a new decision. The probe output never contains a device label (`MediaDeviceInfo.label`, `track.label`): those are personal data.
- **Recording in the webview (ADR-0015, new):**
  - `getUserMedia({audio: true})` is called only on the first click on the microphone, never at startup (PRD §15).
  - `MediaRecorder` with the first supported type of the three above, `audioBitsPerSecond` 64 000, a 1 s `timeslice`. The chunks stay in JS memory until Send.
  - **Size guard:** `audioBitsPerSecond` is only a hint. The recorder counts the chunk bytes as they arrive and sends automatically when **either** the clock reaches 30:00 **or** the bytes reach 48 MiB, whichever comes first (the IPC cap is 64 MiB, and a hidden WKWebView may throttle the 30:00 timer). The observed bytes per second go into `formats.txt`.
  - **Release of the device:** every exit path (Send, Discard, auto-send, any error, the track's `ended`) calls `track.stop()` on all tracks and `AudioContext.close()`, so the OS microphone indicator goes off.
  - **Privacy:** a device label never reaches `console`, the Rust log, an error string, or any runner text file. "Microphone disconnected" shows no device name.
  - Expected output: AAC in MP4 from WKWebView, Opus in WebM or AAC in MP4 from WebView2. Not verified; the probe decides.
  - The duration is the recorder's own clock. A Chromium WebM likely carries no duration header (not verified), so the feed never relies on the file for it.
  - **Alternative not taken:** native recording with `cpal` and an encoder would give one format everywhere. Against it: Opus needs the C libopus build on both OSes, a pure-Rust AAC encoder is not verified to exist, Rust would need its own TCC path, and the webview would still have to play the file.
- **Permissions.**
  - `src-tauri/Info.plist` gets `NSMicrophoneUsageDescription` (English). The file next to `tauri.conf.json` is merged and embedded (verified in tauri-codegen 2.7.3). Per Apple's docs, which I have not re-checked, microphone access without that key kills the process.
  - `src-tauri/Entitlements.plist` gets `com.apple.security.device.audio-input` and is set through `bundle.macOS.entitlements`. Tauri's `hardenedRuntime` defaults to true (tauri-utils 2.10.2). Whether the ad-hoc signature actually carries the runtime flag is not verified. `build-app.sh` **asserts** both keys instead of printing them: `plutil -extract NSMicrophoneUsageDescription raw` on the bundle's `Info.plist` is non-empty, and `codesign -d --entitlements -` contains `com.apple.security.device.audio-input`. CI never reaches TCC with the fake microphone, so a missing key would otherwise be found by the user as a killed process.
  - The webview layer goes through `tauri::Builder::on_permission_request(|webview, kind|)` (verified in tauri 2.12.4): Microphone from the webview labelled `main` gets `Allow`; Camera, DisplayCapture and Geolocation get `Deny`, and so does Microphone from any other label; everything else gets `Default`, which keeps wry's clipboard-read allowance on Windows.
  - This handler is needed. With no handler, wry 0.57.0 on macOS answers `Grant` to every capture type, camera included (verified in `wry_web_view_ui_delegate.rs`), and on Windows WebView2 shows its own prompt.
  - **Not enforced everywhere (verified in wry 0.57.0):** on macOS only camera and microphone capture requests reach the handler; Geolocation and DisplayCapture never do, so their `Deny` is effective on Windows and Linux only. ADR-0015 says so rather than promising a deny on every OS. A page asking for audio and video together is answered by two handler calls; Microphone `Allow` plus Camera `Deny` gives Deny.
  - Windows' own microphone privacy switch is still in force. Which `DOMException` it raises is not verified.
- **Errors (no dialogs, PRD rule 11).** They map from DOMException names:
  - `NotAllowedError` → "access is off" plus `open_microphone_settings`, which opens the fixed URL `x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone` or `ms-settings:privacy-microphone`;
  - `NotFoundError` and `OverconstrainedError` → "no microphone";
  - `NotReadableError` → "unavailable";
  - the track's `ended` event → "disconnected", keeping the bytes;
  - under 1 s → "Too short to save"; the byte threshold or 30:00 → the auto-send above;
  - a send error (`disk_full`) → the bytes stay in memory, and the bar shows the error with "Retry" and "Discard".
- **Store: `send_voice` (`kinige-store`), no migration.** It reuses the stage-1 copy (`.tmp/<process-uuid>/`, sha256, `fsync`). One writer job then renames the file and writes the message (empty body, valid with an attachment), the `attachment` row (`core` with `duration_ms`, `blob_key`, `remote_blob_id` exactly as in §C.6), `attachment_local`, the search index and `mirror_dirty`. It bypasses `draft_attachment`, which has no `duration_ms` column. Order inside the job, as in `commit_staged`: `BEGIN IMMEDIATE`; chat and reply-target checks; insert the message; `fs::rename`; the existing `faults.run_after_rename()` hook; insert `attachment` and `attachment_local`; reindex; `mark_dirty`; clear the reply; commit; `sync_dir`. On an INSERT failure the renamed file is removed. A crash between the rename and the commit leaves an orphan for the existing sweep. The reply line is cleared with `UPDATE draft SET reply_to_id = NULL` followed by `delete_empty_draft`, never `delete_draft`, so a draft row that holds attachment tiles survives. The microphone is **hidden while the draft has attachment tiles** (as in Telegram), so a voice note never goes out beside pending photos and never takes the reply from the entry that follows. Rules:
  - The container is decided by magic bytes, never by `recorder.mimeType`: `ftyp` at offset 4 → `audio/mp4`, `.m4a`; `1A 45 DF A3` → `audio/webm`, `.webm`; anything else → `attachment_unreadable`.
  - `kind = audio` is set explicitly, because `classify` maps `webm` to video (verified in `names.rs`).
  - The name is `voice-YYYY-MM-DD-HHMMSS.<ext>` in local time (chrono).
  - The body is at most 64 MiB (`MAX_PASTE_BYTES`). The product rule lives **only in `send_voice`**: `x-duration-ms` is parsed through `header_text` into a `u64` with `checked_*`, and must be 1 000 to 1 805 000. Below that the error is the new `voice_too_short`; above it, the new `voice_too_long`; a missing, negative, non-numeric or overflowing value is `invalid_input`. The value is the frontend's clock and the user's own data; the domain range below bounds what can be stored.
  - **Domain rule (every existing writer must satisfy it).** `new_attachment` accepts `duration_ms` only for `kind` in {`audio`, `video`}, as `null` or an integer `0 ..= 4_294_967_295_000` (the full range of the Telegram import, which maps a `u32` of seconds to milliseconds). `image`, `file` and later `link` keep `null`; the check sits next to the width/height rule for `image`. Nothing in the import changes, so a 25-hour audiobook still imports and rows written since slice 7 stay valid for slice 11's re-validation.
  - **Search index.** For a file whose name matches the recorder pattern the index text is the single word `voice`, not the stem: `unicode61` splits `voice-2026-11-04-213012` at the dashes, and a search for "11" would return every note of the month.
- **Playback: the `kinige-media://localhost/audio/<uuid>` route.**
  - Only `main` is answered (403 otherwise), GET only, only `kind = 'audio'`, path exactly `/audio/<lowercase uuid v4>` validated before any query. The `Content-Type` comes from an allowlist (`audio/mp4 aac mpeg ogg opus wav flac webm`); any other MIME, `missing` or a purged id gets 404.
  - **Size rule.** Neither the row's `byte_size` nor the file's real length may exceed 64 MiB (`MAX_FULL_BYTES`); otherwise 404, and the player shows "Can't play here · Open in default app" by size as well as by `canPlayType`. The Tauri handler hands the whole body to the responder (no streaming, verified), so this bound is what keeps a 1.5 GB Telegram audio file from being read into memory.
  - **Magic re-probe.** The handler reads the first 12 bytes of the file it opens and requires the container to match the row's MIME (`ftyp` for mp4/aac, EBML for webm, `OggS` for ogg/opus, `ID3` or an `FF Ex` frame for mpeg, `RIFF…WAVE` for wav, `fLaC` for flac); otherwise 404. A row whose file is really `<html…` is never served.
  - **Ranges.** All arithmetic is `u64` with `checked_*`; `Content-Range` uses the file's real length. No `Range` header (or a unit other than `bytes`): 200 with the whole body, `Content-Length` and `Accept-Ranges: bytes`. A single range: 206 with `Content-Range: bytes s-e/len`, an explicit `Content-Length` of `e - s + 1`, and at most 1 MiB per answer (answering fewer bytes than asked is what the Tauri asset protocol does and what WebKit and Chromium accept). A suffix range larger than the file means the whole file; `bytes=0-0` is one byte. `start >= len`, `end < start`, an empty file or a multi-range request gets 416. The body is read with `seek` + `take(n)`; if the read returns fewer bytes than declared (the file changed under the handler) the answer is 416, never a body shorter than its `Content-Length`.
  - Headers as on the other routes: `nosniff`, the sandbox CSP and no `Access-Control-Allow-Origin`.
  - CSP gets `media-src 'self' kinige-media: http://kinige-media.localhost` (prod and dev).
  - The player sets `src` only on the first Play, so opening a chat loads no audio.
- **IPC (§12.3), `main` only:** `send_voice` (raw body; headers `x-chat-id`, `x-duration-ms`, `x-reply-to`) and `open_microphone_settings`. `capture` keeps exactly seven commands; `settings` and `capture` are denied both new commands and the route. No microphone in the capture window.
- **Summaries, copy, MCP.**
  - `AttachmentSummary` gets `voices` and `voice_ms` (ts-rs types regenerate). Strings: "Voice note · 0:40" and "3 voice notes", ru/en.
  - The forest day card gets "· N voice notes" when N > 0. The weight is unchanged (an attachment is +0.5).
  - The Markdown copy writes ` (m:ss)` after an audio link that has a duration. Day files written before 7a that hold Telegram audio keep the old line until the day is next marked dirty; harmless, and noted in the ADR.
  - MCP `AgentAttachment` gets `duration_ms`.
- **Fake microphone for the runners.** An `OscillatorNode` → `MediaStreamAudioDestinationNode` stream replaces `getUserMedia`, so the real `MediaRecorder` and pipeline run with no TCC and no device.
  - **Mechanism that `.env` files cannot reach:** `vite.config.ts` reads `process.env.KINIGE_FAKE_MIC` (Vite does not load `.env` files into the config's `process.env`) and uses `resolve.alias` to point an import such as `virtual:kinige-mic` at `scripts/screens/fake-mic.ts`, a file outside `web/src`; otherwise at `web/src/mic/real.ts`. Without the variable the fake file is not in the production module graph at all.
  - The fake file contains a marker string literal that survives minification (not a comment or an identifier).
  - **Guards:** `scripts/build-app.sh` and `scripts/build-windows.sh` (which `release.yml` runs) grep `web/dist` for the marker after `npm run build` and fail unless `KINIGE_ALLOW_FAKE_MIC=1`; only `screens.yml` sets that, with `KINIGE_FAKE_MIC=1`. `scripts/screens/7a.sh` asserts the marker **is** present in its own `web/dist` (a positive control), and a test shows a build without the opt-in variable fails the guard on a dist that has the marker (a negative control). The grep target is `web/dist`: Tauri embeds the assets compressed, so grepping the `.app` would find nothing.
- **Documents:** ADR-0015; PRD v0.19 (§5 recording details, the §12.3 matrix and route, history); a note in §C.6: `audio` and `video` may carry `duration_ms` (the Telegram import since slice 7, the recorder since 7a), `image`, `file` and later `link` keep `null`, and the bytes and shape of `core` do not change; an `audio` example with a non-null `duration_ms` in the reference vector; `CLAUDE.md` state.

## Not included
- Transcription (P2). A waveform (decision 5). A preview before sending, pausing a recording, a hold or lock gesture, and a hotkey to start recording.
- Converting Ogg or WebM to AAC. Voice notes in the capture window and through ⌥⇧V. Keeping a running recording across a crash or ⌘Q.
- Audio and video players for `video` (P2 video). Saving the playback speed in `settings.json`. A Russian `InfoPlist.strings`.
- Sync and upload of voice files (slice 11). The feed's "media" filter and the search "type" filter (a separate task, as in slice 7).

## What “done” looks like
A 1280 × 800 window.
- **The recording bar** fills the composer slot at the same height: trash `IconButton` (Lucide `Trash2`), a red dot that doesn't pulse (new token `color.recording`, contrast-checked), the timer in tabular figures (it turns warning-colored from 29:00), about 24 level bars from an `AnalyserNode` (Reduce Motion: no bars, only the dot), and Send in the right slot. Focus moves to Send. Enter sends; Escape cancels.
- **The voice player** in a bubble: a round play/pause button (`Play`/`Pause`), a seek `input[type=range]` (←/→ ±5 s), `0:12 / 0:40` (with hours past 59:59) and a `1×` chip. Fixed width between 240 and 360 px, so nothing moves on load. Screen reader label: "Voice note, 40 seconds". The total comes from `duration_ms`, or `audio.duration` if finite, or `–:––`.
- **States:** playing, "File is missing" (the slice 7 tile), "Can't play here · Open in default app", an edited voice note with text under the player, and the trash showing the same player.
- Storybook in en and ru, normal and large text: the recording bar in every error state, the confirm, the warning time, and the player states. Lucide icons (ISC) are added to `icons/README.md`. A 640 px window keeps everything reachable.

## Checks
- **Rust:**
  - `send_voice` on garbage: empty, 1 byte, 1 MiB random, a PNG, 64 MiB + 1, a bad, negative, missing, overflowing or out-of-range `x-duration-ms` (`voice_too_short`, `voice_too_long`), an unknown chat, a missing reply target;
  - `send_voice` with pending tiles and with a reply: the draft row survives when tiles exist, and the reply line is cleared;
  - domain: `new_attachment` with `duration_ms` on an `image` or `file` is invalid; a Telegram export with `duration_seconds` of 90 000 still imports; the §C.6 reference vector gains an `audio` example with a non-null `duration_ms`;
  - WebM is `audio`, not `video`; the name pattern; search for "11" or "2026" does not find a recorder file;
  - fault injection between the rename and the commit, then a sweep: no half rows; injected ENOSPC rolls back;
  - kill survival; search for "voice"; the copy line; the summary; MCP `duration_ms`; `log_privacy`; the `blob_key` property test still holds;
  - the route: 403 for other labels; 404 for an image, a `missing` file and a purged id; a 65 MiB (sparse) `audio` row is 404; a request without `Range` is 200 with `Content-Length` and `Accept-Ranges`; 206, open, suffix (also larger than the file), `bytes=0-0`, `start >= len`, an empty file, a unit other than `bytes`, a multi-range and 416 ranges; the 1 MiB cap; a file truncated between `metadata` and read gives 416, never a short body; the MIME allowlist; a row with `mime = audio/mp4` whose file starts with `<html` is 404;
  - `permission_decision(label, kind)` for every pair above, including the `settings` label and the two-call audio plus video case; `ipc_slice7a.rs` negative tests.
- **Frontend on `MockApi`** with a fake `MediaDevices`: each error, the 5 s confirm, the too-short rule, the auto-send at 30:00, the auto-send at 48 MiB of chunks before 30:00, `track.stop()` and `AudioContext.close()` on every exit path, no device label in anything sent to Rust or logged, the microphone hidden while tiles exist, chat switch, one playback at a time, the speed cycle, no `src` before Play, ru/en keys. Visual baselines go through `visual-update.yml`.
- **Screenshots in `docs/screens/7a/`** (`scripts/screens/7a.sh`, the macOS runner, AX input only, fake-mic build). `seed_demo` with `KINIGE_SEED_VOICE` seeds an `.m4a` made on the runner by `afconvert` from a WAV the seed synthesizes, a Telegram-style `.ogg` (synthetic), a `missing` note, a note with 1:02:03, and an edited note with text. Shots:
  - 01 the feed;
  - 02 recording at ≥ 0:02;
  - 03 the discard confirm;
  - 04 sent;
  - 05 playing (`playback.txt`: the AX value of the elapsed time before and after 2 s);
  - 06 at 2×;
  - 07 after `kill -9`;
  - 08 search;
  - 09 the forest card;
  - 10 en;
  - 11 large text;
  - 12 at 640 px.

  Texts: `mirror.txt`, `files.txt`, `formats.txt`, `bench.txt`. **Windows** (`7a-windows.ps1`): 01 recording, 02 sent and playing; and `formats.txt`.
- **Performance:** `send_voice` p95 < 100 ms for 1 MiB and < 500 ms for 15 MiB; opening a chat with 200 voice notes p95 < 300 ms; cold start < 1.5 s.
- **By hand at acceptance (the user, on the acceptance build; agents never touch TCC or the user's Mac):** the first-recording prompt, a real recording and its playback, unplugging a headset, turning access off in System Settings.

## Touches sensitive areas
**Yes, `sync-crypto-guardian` before merge, but no migration and no new column:**
- a new writer path for synced `attachment` rows, with `duration_ms` coming from the frontend (range-checked in the domain);
- a new protocol route that sits outside the capability system;
- the webview permission handler, which tightens wry's default grant of every capture type;
- two new IPC commands with a raw body.

## Risks
- Untested so far:
  - `getUserMedia` may be missing on the `tauri://` scheme, which is a stop condition;
  - `AudioContext` may not run on a runner without an audio device (fallback: the runner's `.m4a` is sent through a test-only path, and the encoder is checked by hand);
  - autoplay rules for an AX press.
- Format and permission behavior:
  - WKWebView may not play Ogg Opus (Telegram voices) or WebM (a Windows recording after sync); both then get "Can't play here";
  - TCC likely asks again after every ad-hoc-signed update, because the code hash changes (not verified).

## Product decisions (made by the coordinator on 04.11.2026; the user reviews them in the morning)
1. **Gesture:** click to start, Send or Enter to finish; no hold. Holding is awkward on a trackpad and for long notes, and a click is reachable by keyboard and AX.
2. **Cancel:** the trash button or Escape; past 5 s it asks first. A short slip costs nothing, and a long talk is never lost to one key.
3. **Maximum length:** 30 minutes. At 29:00 the timer warns, and at 30:00 the note is sent. That is about 15 MB at 64 kb/s, well inside the 64 MiB IPC cap.
4. **No preview:** Send saves at once, as in Telegram. A bad take is deleted to the trash, which can be undone.
5. **No waveform:** a seek bar, plus a live level meter only while recording. A stored waveform would need a new synced field or an audio decoder, and the bar looks the same for imported notes.
6. **Speed:** 1× → 1.5× → 2×, for the whole session, not saved. These are Telegram's steps, with no settings change.
7. **Text:** none at sending; it is added later with Edit. A pending reply is carried. The microphone shows only with an empty field and no pending attachment tiles (PRD §5, plus the guardian's pre-review).
8. **Copy, search, forest, MCP:**
   - the copy has a link plus `(m:ss)`;
   - search finds the note by the file-name stem "voice" (no transcription until P2);
   - summaries say "Voice note · 0:40" and "N voice notes";
   - MCP gets `duration_ms` and never the bytes.
9. **Telegram voices and other audio:** every `audio` attachment gets the player and counts as a voice note in summaries. They become playable with no migration; a format the webview can't play falls back to "Can't play here".
10. **Edge cases:**
    - under 1 s is not saved;
    - switching chats sends the note to its own chat;
    - hiding the window keeps recording;
    - ⌘Q or a crash loses a running recording;
    - a lost device keeps what was recorded and offers Send.
11. **The capture window gets no microphone,** like attachments in slice 7. §12.3 keeps that window to text.
