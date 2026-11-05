// Rust -> libsodium.js: opens, with libsodium.js and its own byte layout code, what kinige-crypto sealed (the direction the committed
// vectors do not cover: those are libsodium.js -> Rust). Run by tests/js_interop.rs, which writes <dir>/interop.json and <dir>/blob.bin.
//
//   node verify.mjs <dir>

import sodium from 'libsodium-wrappers-sumo';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

await sodium.ready;
const dir = process.argv[2];
if (!dir) throw new Error('usage: node verify.mjs <dir>');
const input = JSON.parse(readFileSync(join(dir, 'interop.json'), 'utf8'));

const bytes = (hex) => sodium.from_hex(hex);
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
const subkey = (master, id, ctx) => sodium.crypto_kdf_derive_from_key(32, id, ctx, master);
const open = (key, nonce, ad, c) => sodium.crypto_aead_xchacha20poly1305_ietf_decrypt(null, c, ad, nonce, key);
const same = (a, b, what) => {
  if (sodium.to_hex(a) !== sodium.to_hex(b)) throw new Error(`${what} differs`);
};

const account = bytes(input.account_id_hex);
const salt = bytes(input.salt);

// Argon2id: the same root from the same password, salt and profile.
const ops = input.profile === 1 ? sodium.crypto_pwhash_OPSLIMIT_MODERATE : sodium.crypto_pwhash_OPSLIMIT_SENSITIVE;
const mem = input.profile === 1 ? sodium.crypto_pwhash_MEMLIMIT_MODERATE : sodium.crypto_pwhash_MEMLIMIT_SENSITIVE;
const pwRoot = sodium.crypto_pwhash(32, input.password, salt, ops, mem, sodium.crypto_pwhash_ALG_ARGON2ID13);
same(pwRoot, bytes(input.pw_root), 'pw_root');
same(subkey(pwRoot, 1, 'kinigePW'), bytes(input.auth_key), 'auth_key');

// The wraps open to the DEK with the KEKs libsodium.js derives.
const dek = bytes(input.dek);
const adPw = concat(text('kinige.wrap.v1'), Uint8Array.of(1), account, u32be(input.key_id), Uint8Array.of(1), salt, Uint8Array.of(input.profile));
const wrapPw = bytes(input.wrap_pw);
same(open(subkey(pwRoot, 2, 'kinigePW'), wrapPw.slice(1, 25), adPw, wrapPw.slice(25)), dek, 'DEK from wrap_pw');
const rcRoot = bytes(input.rc_root);
same(subkey(rcRoot, 1, 'kinigeRC'), bytes(input.rc_auth_key), 'rc_auth_key');
const adRc = concat(text('kinige.wrap.v1'), Uint8Array.of(1), account, u32be(input.key_id), Uint8Array.of(2));
const wrapRc = bytes(input.wrap_rc);
same(open(subkey(rcRoot, 2, 'kinigeRC'), wrapRc.slice(1, 25), adRc, wrapRc.slice(25)), dek, 'DEK from wrap_rc');

// The envelope opens, and a changed header byte does not.
const envelope = bytes(input.envelope);
const headerLen = (envelope[5] << 24) | (envelope[6] << 16) | (envelope[7] << 8) | envelope[8];
const openPart = envelope.slice(0, 9 + headerLen);
const nonce = envelope.slice(9 + headerLen, 33 + headerLen);
const ad = concat(text('kinige.env.v1'), account, openPart);
const kEnv = subkey(dek, 1, 'kinigeDK');
const padded = open(kEnv, nonce, ad, envelope.slice(33 + headerLen));
if (sodium.to_string(sodium.unpad(padded, 1024)) !== input.payload) throw new Error('payload differs');
if (sodium.to_string(openPart.slice(9)) !== input.header) throw new Error('header differs');
const forged = Uint8Array.from(openPart);
forged[12] ^= 1;
let refused = false;
try {
  open(kEnv, nonce, concat(text('kinige.env.v1'), account, forged), envelope.slice(33 + headerLen));
} catch {
  refused = true;
}
if (!refused) throw new Error('a changed header byte was accepted');

// The blob.
const blob = new Uint8Array(readFileSync(join(dir, 'blob.bin')));
const state = sodium.crypto_secretstream_xchacha20poly1305_init_pull(blob.slice(0, 24), bytes(input.blob_key));
const parts = [];
let at = 24;
for (;;) {
  const frame = blob.slice(at, at + (1 << 20) + 17);
  const pulled = sodium.crypto_secretstream_xchacha20poly1305_pull(state, frame, null);
  if (!pulled) throw new Error('chunk does not authenticate');
  parts.push(pulled.message);
  at += frame.length;
  if (pulled.tag === sodium.crypto_secretstream_xchacha20poly1305_TAG_FINAL) break;
}
if (at !== blob.length) throw new Error('bytes after TAG_FINAL');
const file = sodium.unpad(concat(...parts), 1024);
if (file.length !== input.blob_file_size) throw new Error('blob length differs');
console.log('libsodium.js opened the pw_root, both wraps, the envelope and the blob made by Rust');
