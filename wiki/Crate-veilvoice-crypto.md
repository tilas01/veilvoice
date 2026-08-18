![veilvoice-crypto](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-crypto.svg)

# veilvoice-crypto

> Argon2id KDF, X25519+ML-KEM-768 hybrid KEM, XChaCha20-Poly1305 at-rest encryption and page-locked amnesic secrets for VeilVoice.

[[Reference]] &middot; [the same page in the repository](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/README.md)

## Contents

- [What this crate is for](#what-this-crate-is-for)
- [Threat model, stated plainly](#threat-model-stated-plainly)
- [Example](#example)
- [How the crate fits together](#how-the-crate-fits-together)
- [The files](#the-files)

Key derivation, post-quantum-hybrid key agreement, authenticated encryption
and amnesic secret storage for VeilVoice.

## What this crate is for

[[`veilvoice_core`|Crate-veilvoice-core]] makes a voice
unrecognisable; it does not hide the *words*, and it is not meant to. When a
recording needs to stay secret as well — at rest on disk, or in transit to
someone else — that is this crate's job.

- `kdf` — Argon2id, for turning a password into a key.
- `hybrid` — X25519 + ML-KEM-768, so a recording captured today is not
readable by a quantum adversary tomorrow.
- `aead` — XChaCha20-Poly1305, with random nonces and authenticated
associated data.
- `container` — the `.veil` file format that ties the three together.
- `amnesia` — page-locked, zeroizing, constant-time-comparable secrets.
- `shred` — secure erasure, and an honest account of what that is worth
on flash storage.
- `privatefile` — writing a file that is owner-only from the moment it
exists, rather than world-readable until a second syscall tightens it.
- `lock` — the application lock: an Argon2id verifier with a rate limit,
which protects against casual access and says so rather than pretending to
be tamper-proof.

## Threat model, stated plainly

This crate protects data **at rest and in transit** against an attacker who
later obtains the file, including one who stores it until quantum hardware
exists. It does **not** protect against an attacker who is already running
code as you, or who can read this process's memory: page-locking keeps keys
out of the swap file, not out of a debugger. Hibernation writes RAM to disk
wholesale and defeats locking entirely.

## Example

```
use veilvoice_crypto::{container, kdf};

# fn main() -> Result<(), veilvoice_crypto::Error> {
// Cheap parameters so the doctest is fast; real callers use the default.
let params = kdf::KdfParams::weak_for_tests();
let sealed = container::seal_with_password(b"pass phrase", b"audio bytes", params)?;
assert_eq!(container::open_with_password(b"pass phrase", &sealed)?, b"audio bytes");
assert!(container::open_with_password(b"wrong", &sealed).is_err());
# Ok(())
# }
```

VeilVoice contains **no `unsafe` code at all** — including the page-locking
in `amnesia`, which goes through a safe wrapper.

## How the crate fits together

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_lib(["lib.rs<br/>175 lines"])
    n_aead["aead.rs<br/>168 lines"]
    n_amnesia["amnesia.rs<br/>313 lines"]
    n_container["container.rs<br/>479 lines"]
    n_hybrid["hybrid.rs<br/>435 lines"]
    n_kdf["kdf.rs<br/>387 lines"]
    n_lock["lock.rs<br/>732 lines"]
    n_privatefile["privatefile.rs<br/>157 lines"]
    n_shred["shred.rs<br/>401 lines"]
    n_lock --> n_privatefile
```

## The files

| File | Lines | What it is |
|---|---:|---|
| [[`aead.rs`|File-veilvoice-crypto-aead]] | 168 | Authenticated encryption with XChaCha20-Poly1305. |
| [[`amnesia.rs`|File-veilvoice-crypto-amnesia]] | 313 | Amnesic secret storage: page-locked, zeroized, and never printed. |
| [[`container.rs`|File-veilvoice-crypto-container]] | 479 | The .veil encrypted container format. |
| [[`hybrid.rs`|File-veilvoice-crypto-hybrid]] | 435 | Post-quantum hybrid key encapsulation: X25519 + ML-KEM-768. |
| [[`kdf.rs`|File-veilvoice-crypto-kdf]] | 387 | Password-based key derivation with Argon2id. |
| [[`lib.rs`|File-veilvoice-crypto-lib]] | 175 | Key derivation, post-quantum-hybrid key agreement, authenticated encryption and amnesic secret storage for VeilVoice. |
| [[`lock.rs`|File-veilvoice-crypto-lock]] | 732 | The application lock: an Argon2id password verifier with a rate limit. |
| [[`privatefile.rs`|File-veilvoice-crypto-privatefile]] | 157 | Writing a file that only its owner can read. |
| [[`shred.rs`|File-veilvoice-crypto-shred]] | 401 | Secure erasure — the self-destruct. |
