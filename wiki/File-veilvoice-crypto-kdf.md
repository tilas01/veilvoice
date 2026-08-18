![kdf.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-crypto/kdf.svg)

# `crates/veilvoice-crypto/src/kdf.rs`

[[veilvoice-crypto|Crate-veilvoice-crypto]] &middot; 387 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs)

## Contents

- [Cost parameters arrive from a file, so they are hostile input](#cost-parameters-arrive-from-a-file-so-they-are-hostile-input)
- [Domain separation](#domain-separation)
  - [What calls what](#what-calls-what)
  - [Items](#items)

Password-based key derivation with Argon2id.

Argon2id is the memory-hard KDF recommended by RFC 9106 and the OWASP
password-storage guidance; the `id` variant resists both GPU/ASIC
parallelism and the side-channel exposure of pure Argon2i.

Parameters travel *with* the ciphertext rather than being compiled in, so a
file encrypted today still opens after the defaults are raised, and a user
on a small machine can lower the memory cost without forking the format.

# Cost parameters arrive from a file, so they are hostile input

That flexibility has a sharp edge, and two shipped defects came from it.
`m_cost` and `p_cost` are read verbatim from a `.veil` header -- and from the
app-lock file, **which is parsed before anyone has authenticated**.

* `argon2` 0.5.3 evaluates `m_cost < p_cost * 8` *before* it checks whether
`p_cost` is within range, so a large `p_cost` overflows the multiplication.
With overflow checks on -- every debug build, and any project consuming
this crate as a library -- that is a panic on attacker-controlled input
(F-2).
* `m_cost` is allocated before anything else happens, so a header claiming
`u32::MAX` asks for **4 TiB**. The allocation fails, and a failed
allocation aborts the process. Merely *opening* a hostile container killed
the program (F-3).

Both are bounded in `KdfParams::checked`, in arithmetic that cannot
overflow. **Never bypass that funnel.** It is the single place every
derivation passes through, and it exists because the alternative -- checks
scattered across the call sites -- is how one of them gets missed.

A residual is stated rather than fixed: a container may still declare a
legitimate-but-expensive cost, so an attacker can make opening *their* file
slow. That is inherent to shipping the cost with the file, which is what lets
old files open after defaults rise. Slow is not crashing, and the user chose
to open that file.

# Domain separation

The app-lock password and the recording passphrase are different secrets and
are kept different: they are domain-separated in the derivation, so
unlocking the application does not unseal recordings and one cannot be
derived from the other.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_default["KdfParams::default"]
    n_weak_for_tests(["KdfParams::weak_for_tests<br/>pub"])
    n_within(["KdfParams::within<br/>pub"])
    n_checked(["KdfParams::checked<br/>pub"])
    n_build["KdfParams::build"]
    n_derive_key(["derive_key<br/>pub"])
    n_random_salt(["random_salt<br/>pub"])
    n_build --> n_checked
    n_within --> n_checked
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `KdfParams` <sub>pub struct</sub> | [51](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L51) | Argon2id cost parameters. |
| `KdfParams::default` <sub>fn</sub> | [65](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L65) | RFC 9106's "first recommended" profile: 2 GiB is the second option, but 256 MiB with three passes is the sweet spot for an interactive desktop unlock — strong against offline cracking while still opening a file in well under a second on ordinary hardware. |
| `KdfParams::weak_for_tests` <sub>pub fn</sub> | [78](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L78) | A deliberately cheap profile for tests and low-memory devices. |
| `KdfParams::MAX_P_COST` <sub>const</sub> | [87](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L87) | Argon2's own documented ceiling on parallelism: 2^24 - 1. |
| `KdfParams::MAX_M_COST` <sub>pub const</sub> | [108](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L108) | The largest memory cost this build will attempt, in KiB — 4 GiB. |
| `KdfParams::UNATTENDED_MAX_M_COST` <sub>pub const</sub> | [124](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L124) | A ceiling for a caller with nobody watching. |
| `KdfParams::within` <sub>pub fn</sub> | [136](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L136) | Check the costs against a caller-chosen memory ceiling as well as the built-in one. |
| `KdfParams::checked` <sub>pub fn</sub> | [165](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L165) | Check the costs are ones Argon2 can accept, before handing them to it. |
| `KdfParams::build` <sub>fn</sub> | [185](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L185) | Reject values Argon2 cannot accept, so a corrupt header fails loudly rather than panicking deep inside the KDF. |
| `SALT_LEN` <sub>pub const</sub> | [194](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L194) | Length of the salt stored in an encrypted container. |
| `KEY_LEN` <sub>pub const</sub> | [196](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L196) | Length of a derived symmetric key. |
| `derive_key` <sub>pub fn</sub> | [202](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L202) | Derive a 32-byte key from password and salt. |
| `random_salt` <sub>pub fn</sub> | [215](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-crypto/src/kdf.rs#L215) | Draw a fresh random salt from the OS CSPRNG. |
