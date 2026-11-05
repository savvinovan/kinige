// Test vectors for kinige-crypto (ADR-0001 appendix B, S4b items 1 and 12), made with libsodium.js so that byte compatibility with a
// future web client is proven and not assumed. Every input is fixed and obviously fake: no real password, words, key or text.
//
//   npm ci && npm run vectors     writes vectors.json and blobs/*.bin (the blobs are random: a stream header is, so they change
//                                 on every run; regenerate them only when the format changes)
//   npm run check                 rebuilds everything but the blobs and compares with vectors.json; decrypts the committed blobs
//
// The Rust tests (crates/kinige-crypto/src/vectors_tests.rs) read vectors.json and the blobs.

import sodium from 'libsodium-wrappers-sumo';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

await sodium.ready;
// The vectors live next to the crate whose tests read them; this directory only holds the generator and its npm lock file.
const here = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'crates', 'kinige-crypto', 'vectors');
const CHECK = process.argv.includes('--check');
const MIB = 1 << 20;

const hex = (bytes) => sodium.to_hex(bytes);
const bytesOf = (start, length = 32) => Uint8Array.from({ length }, (_, i) => (start + i) & 0xff);
const text = (s) => sodium.from_string(s);
const concat = (...parts) => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
};
const u32be = (n) => Uint8Array.of((n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255);
const uuidBytes = (uuid) => sodium.from_hex(uuid.replaceAll('-', ''));

// RFC 8785 as restricted by ADR-0001 appendix A.0: strings, booleans, null, arrays, objects, integers. Keys sort by UTF-16 code
// units (the default of Array.prototype.sort on strings); strings are serialized by JSON.stringify (ECMAScript, well-formed).
function jcs(value) {
  if (value === null || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) throw new Error('only integers within 2^53');
    return String(value);
  }
  if (typeof value === 'string') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(jcs).join(',')}]`;
  const keys = Object.keys(value).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${jcs(value[k])}`).join(',')}}`;
}

// ---- fixed inputs --------------------------------------------------------------------------------------------------------------
const ACCOUNT = '11111111-1111-4111-8111-111111111111';
const DEVICE = '66666666-6666-4666-8666-666666666666';
const MESSAGE = '33333333-3333-4333-8333-333333333333';
const OP = '55555555-5555-4555-8555-555555555555';
const KEY_ID = 1;
const PASSWORD = 'fake test password, not a real one';
const SALT = bytesOf(1, 16);
const DEK = bytesOf(0x40);
const SERVER_SECRET = bytesOf(0x60);
const LOGIN = 'fake.user@example.test';
const hlc = (wall, counter) => `${String(wall).padStart(13, '0')}-${String(counter).padStart(5, '0')}-${DEVICE}`;
const H_CORE = hlc(1762300000000, 0);
const H_BODY = hlc(1762300000500, 1);

// The words of the official BIP-39 test vectors for 256-bit entropy (Trezor's vectors.json); the Rust test parses the phrases and
// checks the entropy, so a wrong phrase fails there.
const WORDS = [
  { entropy: '00'.repeat(32), phrase: `${'abandon '.repeat(23)}art` },
  {
    entropy: '7f'.repeat(32),
    phrase: 'legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title',
  },
  {
    entropy: '80'.repeat(32),
    phrase: 'letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless',
  },
  { entropy: 'ff'.repeat(32), phrase: `${'zoo '.repeat(23)}vote` },
];

// Strings that exercise canonical JSON: non-ASCII, a surrogate pair, quotes, backslash, control characters, DEL, U+2028, '<>&/'.
const NASTY = `Privet \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442} ${String.fromCodePoint(0x1f642)} "q" \\ \n\t\r\b\f ${String.fromCharCode(1, 0x1f)} ${String.fromCharCode(0x7f)} ${String.fromCharCode(0x2028)} <>&/ caf${String.fromCharCode(0xe9)}`;
const BODY_GROUP = jcs({ base: null, body_md: NASTY, edited_at: null, supersedes: [] });
const CORE_GROUP = jcs({ created_at: 1762300000000, created_tz: 'Europe/Moscow', origin: 'user', reply_to_id: null });

const aeadSeal = (key, nonce, ad, m) => sodium.crypto_aead_xchacha20poly1305_ietf_encrypt(m, ad, null, nonce, key);
const subkey = (master, id, ctx) => sodium.crypto_kdf_derive_from_key(32, id, ctx, master);
const pwhash = (profile) =>
  profile === 1
    ? sodium.crypto_pwhash(32, PASSWORD, SALT, sodium.crypto_pwhash_OPSLIMIT_MODERATE, sodium.crypto_pwhash_MEMLIMIT_MODERATE, sodium.crypto_pwhash_ALG_ARGON2ID13)
    : sodium.crypto_pwhash(32, PASSWORD, SALT, sodium.crypto_pwhash_OPSLIMIT_SENSITIVE, sodium.crypto_pwhash_MEMLIMIT_SENSITIVE, sodium.crypto_pwhash_ALG_ARGON2ID13);

function buildVectors() {
  const account = uuidBytes(ACCOUNT);
  const out = { libsodium_js: { package: 'libsodium-wrappers-sumo', version: sodium.sodium_version_string() } };

  // Keys (§8.1)
  const pwRoot1 = pwhash(1);
  const pwRoot2 = pwhash(2);
  const rcRoot = sodium.from_hex(WORDS[0].entropy);
  out.keys = {
    password: PASSWORD,
    salt: hex(SALT),
    pw_root_profile1: hex(pwRoot1),
    pw_root_profile2: hex(pwRoot2),
    auth_key: hex(subkey(pwRoot1, 1, 'kinigePW')),
    kek_pw: hex(subkey(pwRoot1, 2, 'kinigePW')),
    rc_root: hex(rcRoot),
    rc_auth_key: hex(subkey(rcRoot, 1, 'kinigeRC')),
    kek_rc: hex(subkey(rcRoot, 2, 'kinigeRC')),
    dek: hex(DEK),
    k_env: hex(subkey(DEK, 1, 'kinigeDK')),
    k_reserved: hex(subkey(DEK, 2, 'kinigeDK')),
    k_rev: hex(subkey(DEK, 3, 'kinigeDK')),
    auth_hash: hex(sodium.crypto_generichash(32, subkey(pwRoot1, 1, 'kinigePW'))),
    server_secret: hex(SERVER_SECRET),
    login: LOGIN,
    unknown_login_salt: hex(sodium.crypto_generichash(16, text(LOGIN), SERVER_SECRET)),
  };
  out.words = WORDS;

  // Wraps (§A.1)
  const kekPw = subkey(pwRoot1, 2, 'kinigePW');
  const kekRc = subkey(rcRoot, 2, 'kinigeRC');
  const adPw = concat(text('kinige.wrap.v1'), Uint8Array.of(1), account, u32be(KEY_ID), Uint8Array.of(1), SALT, Uint8Array.of(1));
  const adRc = concat(text('kinige.wrap.v1'), Uint8Array.of(1), account, u32be(KEY_ID), Uint8Array.of(2));
  const noncePw = bytesOf(0xb0, 24);
  const nonceRc = bytesOf(0xc0, 24);
  out.wraps = {
    account_id: ACCOUNT,
    key_id: KEY_ID,
    profile: 1,
    ad_wrap_pw: hex(adPw),
    ad_wrap_rc: hex(adRc),
    nonce_pw: hex(noncePw),
    nonce_rc: hex(nonceRc),
    wrap_pw: hex(concat(Uint8Array.of(1), noncePw, aeadSeal(kekPw, noncePw, adPw, DEK))),
    wrap_rc: hex(concat(Uint8Array.of(1), nonceRc, aeadSeal(kekRc, nonceRc, adRc, DEK))),
  };

  // Padding (§5.1)
  out.padding = [0, 1, 1023, 1024, 1025, 2047].map((length) => {
    const padded = sodium.pad(Uint8Array.from({ length }, (_, i) => (i * 7 + 3) & 0xff), 1024);
    return { input_length: length, pattern: 'byte i = (i * 7 + 3) mod 256', padded_length: padded.length, padded_blake2b256: hex(sodium.crypto_generichash(32, padded)) };
  });

  // Envelopes (§5, §A.1)
  const envKey = subkey(DEK, 1, 'kinigeDK');
  const upsertHeader = `{"entity":"message","id":"${MESSAGE}","op":"${OP}","kind":"upsert","groups":{"body":{"h":"${H_BODY}","sv":1},"core":{"h":"${H_CORE}","sv":1}}}`;
  const upsertPayload = `{"groups":{"body":${BODY_GROUP},"core":${CORE_GROUP}}}`;
  const purgedHeader = `{"entity":"message","id":"${MESSAGE}","op":"${OP}","kind":"purged","purged_of":{"body":"${H_BODY}","core":"${H_CORE}"}}`;
  const envelope = (header, payload, nonce) => {
    const open = concat(Uint8Array.of(1), u32be(KEY_ID), u32be(text(header).length), text(header));
    const ad = concat(text('kinige.env.v1'), account, open);
    const sealed = aeadSeal(envKey, nonce, ad, sodium.pad(text(payload), 1024));
    return { ad, bytes: concat(open, nonce, sealed) };
  };
  const nonceUpsert = bytesOf(0xa0, 24);
  const noncePurged = bytesOf(0xd0, 24);
  const upsert = envelope(upsertHeader, upsertPayload, nonceUpsert);
  const purged = envelope(purgedHeader, '', noncePurged);
  out.envelopes = {
    account_id: ACCOUNT,
    key_id: KEY_ID,
    upsert: { header: upsertHeader, payload: upsertPayload, nonce: hex(nonceUpsert), ad_env: hex(upsert.ad), envelope: hex(upsert.bytes), body_group: BODY_GROUP, core_group: CORE_GROUP },
    purged: { header: purgedHeader, payload: '', nonce: hex(noncePurged), ad_env: hex(purged.ad), envelope: hex(purged.bytes) },
  };

  // The id and the content of a `conflict` revision (§1, §3.4, B items 1 and 12)
  const group = 1; // body
  const idInput = concat(uuidBytes(MESSAGE), Uint8Array.of(group), text(H_BODY));
  const hash = sodium.crypto_generichash(32, idInput, subkey(DEK, 3, 'kinigeDK'));
  const idBytes = hash.slice(0, 16);
  idBytes[6] = (idBytes[6] & 0x0f) | 0x40;
  idBytes[8] = (idBytes[8] & 0x3f) | 0x80;
  const id = hex(idBytes).replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5');
  const snapshot = jcs({ g: 'body', h: H_BODY, raw: BODY_GROUP, sv: 1 });
  const core = jcs({ message_id: MESSAGE, reason: 'conflict', saved_at: 1762300000500, snapshot_json: snapshot });
  out.revision = { message_id: MESSAGE, group, loser_hlc: H_BODY, loser_sv: 1, loser_raw: BODY_GROUP, id_input: hex(idInput), id_hash32: hex(hash), id, snapshot_json: snapshot, core };

  // Canonical JSON (B item 12): the value is built here from an object and serialized by the rules above.
  out.canonical = [
    { value_description: 'body group with nasty text', canonical: BODY_GROUP },
    { value_description: 'core group', canonical: CORE_GROUP },
    { value_description: 'keys sorted by UTF-16 code units, an astral key after a BMP key', canonical: jcs({ [String.fromCodePoint(0x1f642)]: 1, [String.fromCharCode(0xff5e)]: 2, b: [true, false, null], a: { z: 0, y: 9007199254740991 } }) },
  ];
  return out;
}

// ---- blobs (§7, §A.2) ----------------------------------------------------------------------------------------------------------
const BLOB_KEY = bytesOf(0x50);
const BLOB_SIZES = [0, MIB - 1, MIB, MIB + 1];
const filePattern = (size) => Uint8Array.from({ length: size }, (_, i) => i % 251);

function sealBlob(file, key) {
  const padded = sodium.pad(file, 1024);
  const { state, header } = sodium.crypto_secretstream_xchacha20poly1305_init_push(key);
  const frames = [header];
  for (let at = 0; at < padded.length; at += MIB) {
    const last = at + MIB >= padded.length;
    const tag = last ? sodium.crypto_secretstream_xchacha20poly1305_TAG_FINAL : sodium.crypto_secretstream_xchacha20poly1305_TAG_MESSAGE;
    frames.push(sodium.crypto_secretstream_xchacha20poly1305_push(state, padded.slice(at, at + MIB), null, tag));
  }
  return concat(...frames);
}

function openBlob(blob, key) {
  const state = sodium.crypto_secretstream_xchacha20poly1305_init_pull(blob.slice(0, 24), key);
  const parts = [];
  let at = 24;
  for (;;) {
    const frame = blob.slice(at, at + MIB + 17);
    const pulled = sodium.crypto_secretstream_xchacha20poly1305_pull(state, frame, null);
    if (!pulled) throw new Error('chunk does not authenticate');
    parts.push(pulled.message);
    at += frame.length;
    if (pulled.tag === sodium.crypto_secretstream_xchacha20poly1305_TAG_FINAL) break;
  }
  if (at !== blob.length) throw new Error('bytes after TAG_FINAL');
  return sodium.unpad(concat(...parts), 1024);
}

function blobVectors() {
  mkdirSync(join(here, 'blobs'), { recursive: true });
  return BLOB_SIZES.map((size) => {
    const file = filePattern(size);
    const blob = sealBlob(file, BLOB_KEY);
    writeFileSync(join(here, 'blobs', `blob-${size}.bin`), blob);
    return { file_size: size, pattern: 'byte i = i mod 251', blob_key: hex(BLOB_KEY), blob_file: `blobs/blob-${size}.bin`, blob_length: blob.length, file_blake2b256: hex(sodium.crypto_generichash(32, file)) };
  });
}

function checkBlobs(entries) {
  for (const entry of entries) {
    const blob = new Uint8Array(readFileSync(join(here, entry.blob_file)));
    const file = openBlob(blob, BLOB_KEY);
    if (file.length !== entry.file_size || hex(sodium.crypto_generichash(32, file)) !== entry.file_blake2b256 || blob.length !== entry.blob_length) {
      throw new Error(`blob ${entry.blob_file} does not match its entry`);
    }
  }
}

// ---- main ----------------------------------------------------------------------------------------------------------------------
const vectors = buildVectors();
const target = join(here, 'vectors.json');
if (CHECK) {
  const committed = JSON.parse(readFileSync(target, 'utf8'));
  const blobs = committed.blobs;
  delete committed.blobs;
  const now = JSON.parse(JSON.stringify(vectors));
  if (JSON.stringify(committed) !== JSON.stringify(now)) {
    console.error('vectors.json differs from what libsodium.js makes now');
    process.exit(1);
  }
  checkBlobs(blobs);
  console.log('vectors.json matches libsodium.js', sodium.sodium_version_string(), 'and the committed blobs decrypt');
} else {
  vectors.blobs = blobVectors();
  writeFileSync(target, `${JSON.stringify(vectors, null, 2)}\n`);
  console.log('wrote vectors.json and', vectors.blobs.length, 'blobs with libsodium', sodium.sodium_version_string());
}
