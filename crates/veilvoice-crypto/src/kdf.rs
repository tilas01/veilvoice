// SPDX-License-Identifier: GPL-3.0-or-later
//! Password-based key derivation with Argon2id.
//!
//! Argon2id is the memory-hard KDF recommended by RFC 9106 and the OWASP
//! password-storage guidance; the `id` variant resists both GPU/ASIC
//! parallelism and the side-channel exposure of pure Argon2i.
//!
//! Parameters travel *with* the ciphertext rather than being compiled in, so a
//! file encrypted today still opens after the defaults are raised, and a user
//! on a small machine can lower the memory cost without forking the format.
//!
//! # Cost parameters arrive from a file, so they are hostile input
//!
//! That flexibility has a sharp edge, and two shipped defects came from it.
//! `m_cost` and `p_cost` are read verbatim from a `.veil` header -- and from the
//! app-lock file, **which is parsed before anyone has authenticated**.
//!
//! * `argon2` 0.5.3 evaluates `m_cost < p_cost * 8` *before* it checks whether
//!   `p_cost` is within range, so a large `p_cost` overflows the multiplication.
//!   With overflow checks on -- every debug build, and any project consuming
//!   this crate as a library -- that is a panic on attacker-controlled input
//!   (F-2).
//! * `m_cost` is allocated before anything else happens, so a header claiming
//!   `u32::MAX` asks for **4 TiB**. The allocation fails, and a failed
//!   allocation aborts the process. Merely *opening* a hostile container killed
//!   the program (F-3).
//!
//! Both are bounded in [`KdfParams::checked`], in arithmetic that cannot
//! overflow. **Never bypass that funnel.** It is the single place every
//! derivation passes through, and it exists because the alternative -- checks
//! scattered across the call sites -- is how one of them gets missed.
//!
//! A residual is stated rather than fixed: a container may still declare a
//! legitimate-but-expensive cost, so an attacker can make opening *their* file
//! slow. That is inherent to shipping the cost with the file, which is what lets
//! old files open after defaults rise. Slow is not crashing, and the user chose
//! to open that file.
//!
//! # Domain separation
//!
//! The app-lock password and the recording passphrase are different secrets and
//! are kept different: they are domain-separated in the derivation, so
//! unlocking the application does not unseal recordings and one cannot be
//! derived from the other.
//!
//! # In plain words
//!
//! This turns a passphrase into a key.
//!
//! A passphrase somebody can remember is far too short and too predictable to use
//! directly, so it is put through a process designed to be slow and to need a
//! large amount of memory. That does not slow you down noticeably once, but it
//! makes guessing millions of passphrases enormously expensive for anybody trying.
//!
//! The settings are stored with each file, so an old recording still opens after
//! the defaults are made stronger.

use crate::{Error, Secret};
use argon2::{Algorithm, Argon2, Params, Version};

/// Argon2id cost parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in kibibytes.
    pub m_cost: u32,
    /// Number of passes.
    pub t_cost: u32,
    /// Degree of parallelism.
    pub p_cost: u32,
}

impl Default for KdfParams {
    /// RFC 9106's "first recommended" profile: 2 GiB is the second option, but
    /// 256 MiB with three passes is the sweet spot for an interactive desktop
    /// unlock — strong against offline cracking while still opening a file in
    /// well under a second on ordinary hardware.
    fn default() -> Self {
        Self {
            m_cost: 256 * 1024,
            t_cost: 3,
            p_cost: 4,
        }
    }
}

impl KdfParams {
    /// A deliberately cheap profile for tests and low-memory devices.
    ///
    /// Do not use this to protect real data.
    pub fn weak_for_tests() -> Self {
        Self {
            m_cost: 8,
            t_cost: 1,
            p_cost: 1,
        }
    }

    /// Argon2's own documented ceiling on parallelism: 2^24 - 1.
    const MAX_P_COST: u32 = 0x00ff_ffff;

    /// The largest memory cost this build will attempt, in KiB — 4 GiB.
    ///
    /// A ceiling is necessary because `m_cost` arrives from the file. Argon2
    /// allocates that much memory before it does anything else, so a header
    /// claiming `u32::MAX` asks for four *terabytes*: the allocation fails, and
    /// a failed allocation in Rust aborts the process. Merely *attempting to
    /// open* a hostile `.veil` would kill the program, and for the app lock it
    /// is worse — that file is read before anyone has authenticated, so
    /// anything that can write it can stop VeilVoice from starting at all.
    ///
    /// 4 GiB is chosen to sit well above every parameter set anyone would
    /// deliberately pick: RFC 9106's *first* recommended profile is 2 GiB and
    /// this crate's default is 256 MiB. A file declaring more than this is
    /// refused with [`Error::KdfParams`] rather than obeyed.
    ///
    /// The honest residual: a file whose declared cost is legitimate but larger
    /// than *this machine's* memory still cannot be opened, and that failure
    /// comes from the allocator rather than from here. A cap cannot fix a
    /// small machine; it can stop an absurd number from being taken seriously.
    pub const MAX_M_COST: u32 = 4 * 1024 * 1024;

    /// The largest number of passes this build will attempt.
    ///
    /// **F-82.** `m_cost` had a ceiling and `t_cost` had only a test for zero,
    /// so a header could declare `u32::MAX` passes: four billion of them, over
    /// however much memory it also asked for. Nothing overflows and nothing
    /// allocates, so every check above passed and the derivation simply did not
    /// finish.
    ///
    /// Found by the coverage-guided campaign, which produced a header
    /// declaring `m_cost` 65535, `t_cost` 4521984 and `p_cost` 1280. Measured
    /// on the machine that found it, in a release build: **about 74 hours**,
    /// and that input is not the worst one, only the one the fuzzer happened
    /// to reach. `u32::MAX` passes at the same memory is roughly eight years.
    ///
    /// It matters in two places and the second is worse. A `.veil` file is
    /// something somebody sent you, and merely attempting to open it would hang
    /// the program. The app-lock file carries the same three numbers and is
    /// read **before anyone has authenticated**, so anything able to write it
    /// could stop VeilVoice from starting, for ever, with no error and nothing
    /// to see. That is the argument [`MAX_M_COST`](Self::MAX_M_COST) already
    /// makes about memory; nobody had made it about time.
    ///
    /// 16 is chosen the way the memory ceiling was, and then tighter, because
    /// time has no allocator to fail on its behalf. RFC 9106's two recommended
    /// profiles use one pass and three, libsodium's most expensive preset uses
    /// four, and this crate's default is three; 16 is four times the highest of
    /// those. Measured in a release build: 16 passes at
    /// [`MAX_M_COST`](Self::MAX_M_COST) is 75 seconds, and at
    /// [`UNATTENDED_MAX_M_COST`](Self::UNATTENDED_MAX_M_COST) it is 18. So the
    /// most expensive header this build will accept is a wait somebody can sit
    /// through, rather than one they will never see the end of.
    ///
    /// This is one ceiling and it is enforced in [`checked`](Self::checked),
    /// the single funnel every derivation passes through, so it holds for the
    /// container, for the app lock, and for anything built against this crate.
    /// The honest residual is the same one the memory ceiling states: a cap
    /// cannot make a hostile file cheap, it can stop an absurd number from
    /// being taken seriously.
    pub const MAX_T_COST: u32 = 16;

    /// A ceiling for a caller with nobody watching.
    ///
    /// [`MAX_M_COST`](Self::MAX_M_COST) exists to stop an *absurd* value; it is
    /// deliberately generous, so a container may still declare a legitimate but
    /// expensive cost and make itself slow to open. That is fine when a person
    /// chose to open that file and can decide to stop waiting. It is not fine
    /// for a service processing whatever arrives, which is why
    /// [`KdfParams::within`] exists and this is the value to pass it: 1 GiB is
    /// four times this crate's default and still opens in a few seconds, while
    /// refusing a header that asks for four gigabytes of someone else's memory.
    ///
    /// This is a *policy*, not a security boundary — the honest framing is that
    /// it bounds the cost of being handed a hostile file, not that it makes one
    /// safe.
    pub const UNATTENDED_MAX_M_COST: u32 = 1024 * 1024;

    /// Check the costs against a caller-chosen memory ceiling as well as the
    /// built-in one.
    ///
    /// Opening a container whose declared cost is legitimate but large is slow
    /// by design, and that is the price of shipping the cost with the file so
    /// old files keep opening. A caller running without a human present — a
    /// batch job, a service, anything processing files it did not choose — can
    /// use this to decline instead of spending the memory. Pass
    /// [`UNATTENDED_MAX_M_COST`](Self::UNATTENDED_MAX_M_COST) unless there is a
    /// reason for something else.
    ///
    /// There is no time ceiling here and there does not need to be: unlike
    /// memory, time is bounded for every caller by
    /// [`MAX_T_COST`](Self::MAX_T_COST), which is tight enough that the worst
    /// header this build will accept is a wait rather than a hang. A ceiling
    /// here as well would sit on the *attended* path too, since that is
    /// [`super::container::open_with_password`] with a larger number, and would
    /// refuse a container somebody had deliberately chosen to open.
    pub fn within(&self, max_m_cost: u32) -> Result<(), Error> {
        self.checked()?;
        if self.m_cost > max_m_cost {
            return Err(Error::KdfCostRefused {
                requested: self.m_cost,
                ceiling: max_m_cost,
            });
        }
        Ok(())
    }

    /// Check the costs are ones Argon2 can accept, **before** handing them to
    /// it.
    ///
    /// This is not belt-and-braces, it is a fix. `argon2` 0.5.3 validates in
    /// the wrong order: `Params::new` evaluates `m_cost < p_cost * 8` before it
    /// checks `p_cost > MAX_P_COST`, so a `p_cost` above `u32::MAX / 8`
    /// overflows the multiplication. With overflow checks on — every debug
    /// build, and any consumer of this crate as a library — that is a **panic
    /// on attacker-controlled input**, since `p_cost` is read verbatim from a
    /// `.veil` header or an app-lock file. Found by the campaign in
    /// `tests/parser_fuzz.rs`.
    ///
    /// VeilVoice's own release profile disables overflow checks, where the
    /// multiplication wraps and the `MAX_P_COST` test then rejects it anyway —
    /// but "our release profile happens to make the panic unreachable" is not a
    /// property to rely on, and it is not true for anyone building against
    /// these crates. So the bound is enforced here, in the one place every
    /// derivation passes through, in arithmetic that cannot overflow.
    pub fn checked(&self) -> Result<(), Error> {
        if self.p_cost == 0 || self.p_cost > Self::MAX_P_COST {
            return Err(Error::KdfParams);
        }
        // F-82. Zero is not a number of passes, and neither is four billion:
        // a header declaring `u32::MAX` passes nothing overflows, nothing
        // allocates, and nothing finishes.
        if self.t_cost == 0 || self.t_cost > Self::MAX_T_COST {
            return Err(Error::KdfParams);
        }
        if self.m_cost > Self::MAX_M_COST {
            return Err(Error::KdfParams);
        }
        // Widened to u64 deliberately: this is the multiplication that
        // overflows upstream.
        if u64::from(self.m_cost) < u64::from(self.p_cost) * 8 {
            return Err(Error::KdfParams);
        }
        Ok(())
    }

    /// Reject values Argon2 cannot accept, so a corrupt header fails loudly
    /// rather than panicking deep inside the KDF.
    fn build(&self, out_len: usize) -> Result<Argon2<'static>, Error> {
        self.checked()?;
        let params = Params::new(self.m_cost, self.t_cost, self.p_cost, Some(out_len))
            .map_err(|_| Error::KdfParams)?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// Length of the salt stored in an encrypted container.
pub const SALT_LEN: usize = 16;
/// Length of a derived symmetric key.
pub const KEY_LEN: usize = 32;

/// Derive a 32-byte key from `password` and `salt`.
///
/// The result lands directly in page-locked, zeroizing storage; it is never
/// held in an ordinary `Vec` along the way.
pub fn derive_key(password: &[u8], salt: &[u8], params: KdfParams) -> Result<Secret, Error> {
    if salt.len() < 8 {
        return Err(Error::KdfParams);
    }
    let mut key = Secret::zeroed(KEY_LEN);
    params
        .build(KEY_LEN)?
        .hash_password_into(password, salt, key.expose_mut())
        .map_err(|_| Error::Kdf)?;
    Ok(key)
}

/// Draw a fresh random salt from the OS CSPRNG.
pub fn random_salt() -> Result<[u8; SALT_LEN], Error> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).map_err(|_| Error::Random)?;
    Ok(salt)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &[u8] = b"correct horse battery staple";

    fn weak() -> KdfParams {
        KdfParams::weak_for_tests()
    }

    #[test]
    fn derivation_is_deterministic() {
        let salt = [9u8; SALT_LEN];
        let a = derive_key(P, &salt, weak()).unwrap();
        let b = derive_key(P, &salt, weak()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), KEY_LEN);
    }

    #[test]
    fn different_password_or_salt_diverges() {
        let base = derive_key(P, &[9u8; SALT_LEN], weak()).unwrap();
        assert_ne!(
            base,
            derive_key(b"wrong", &[9u8; SALT_LEN], weak()).unwrap()
        );
        assert_ne!(base, derive_key(P, &[10u8; SALT_LEN], weak()).unwrap());
    }

    #[test]
    fn cost_parameters_change_the_key() {
        // Parameters are part of the derivation, so a header that lies about
        // them cannot yield the right key.
        let salt = [3u8; SALT_LEN];
        let a = derive_key(
            P,
            &salt,
            KdfParams {
                t_cost: 1,
                ..weak()
            },
        )
        .unwrap();
        let b = derive_key(
            P,
            &salt,
            KdfParams {
                t_cost: 2,
                ..weak()
            },
        )
        .unwrap();
        assert_ne!(a, b);
    }

    /// **F-82.** The exact parameters the coverage-guided campaign found, and
    /// the worst case it did not reach.
    ///
    /// These pass every other check: nothing overflows, nothing allocates
    /// beyond the memory ceiling, and `m_cost >= p_cost * 8` holds. The only
    /// thing wrong with them is that the derivation does not finish. Measured
    /// in a release build before the ceiling existed: about 74 hours for the
    /// first set, and roughly eight years for `u32::MAX`.
    ///
    /// The numbers are written out rather than referred to, because that is
    /// what makes this a regression test for the input rather than a
    /// restatement of the rule.
    #[test]
    fn a_number_of_passes_that_would_not_finish_is_refused() {
        let found = KdfParams {
            m_cost: 65535,
            t_cost: 4_521_984,
            p_cost: 1280,
        };
        assert!(
            matches!(found.checked(), Err(Error::KdfParams)),
            "{found:?}"
        );
        assert!(
            matches!(
                derive_key(P, &[0u8; SALT_LEN], found),
                Err(Error::KdfParams)
            ),
            "the derivation must refuse, not run"
        );

        for t_cost in [u32::MAX, u32::MAX / 2, KdfParams::MAX_T_COST + 1] {
            let bad = KdfParams { t_cost, ..weak() };
            assert!(
                matches!(bad.checked(), Err(Error::KdfParams)),
                "t_cost {t_cost} was accepted"
            );
        }

        // And the ceiling itself still works, so this refuses absurd values
        // rather than everything.
        let at_the_line = KdfParams {
            t_cost: KdfParams::MAX_T_COST,
            ..weak()
        };
        assert!(at_the_line.checked().is_ok(), "{at_the_line:?}");
    }

    /// Every parameter set this project actually writes stays acceptable, and
    /// the ceiling sits well clear of them.
    ///
    /// A ceiling that refused the default would be a much louder bug than the
    /// one it was added for, and "we chose a number" is not evidence that the
    /// number is above the ones in use.
    #[test]
    fn the_time_ceiling_sits_above_every_cost_this_project_uses() {
        for params in [KdfParams::default(), KdfParams::weak_for_tests()] {
            assert!(params.checked().is_ok(), "{params:?}");
            assert!(
                params.t_cost * 4 <= KdfParams::MAX_T_COST,
                "{params:?} leaves no room under the ceiling"
            );
        }
        // The attended path must still reach the ceiling: a bound that nothing
        // can get to is a bound on nothing.
        let expensive = KdfParams {
            t_cost: KdfParams::MAX_T_COST,
            ..KdfParams::default()
        };
        assert!(
            expensive.within(KdfParams::MAX_M_COST).is_ok(),
            "{expensive:?}"
        );
    }

    #[test]
    fn key_material_is_page_locked_storage() {
        let k = derive_key(P, &[1u8; SALT_LEN], weak()).unwrap();
        assert!(format!("{k:?}").contains("redacted"));
    }

    #[test]
    fn impossible_parameters_are_rejected_not_panicked() {
        let bad = KdfParams {
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
        };
        assert!(matches!(
            derive_key(P, &[0u8; SALT_LEN], bad),
            Err(Error::KdfParams)
        ));
    }

    /// Regression for the panic the parser campaign found: `argon2` 0.5.3
    /// computes `p_cost * 8` before checking `p_cost`'s ceiling, so a `p_cost`
    /// above `u32::MAX / 8` overflows. `p_cost` comes straight out of a `.veil`
    /// header or an app-lock file, so this is attacker-controlled.
    #[test]
    fn an_absurd_parallelism_is_rejected_rather_than_overflowing() {
        for p_cost in [
            u32::MAX,
            u32::MAX / 8 + 1,
            0x2000_0000,
            KdfParams::MAX_P_COST + 1,
        ] {
            let bad = KdfParams {
                m_cost: 1 << 20,
                t_cost: 3,
                p_cost,
            };
            assert!(bad.checked().is_err(), "p_cost {p_cost} accepted");
            assert!(
                matches!(derive_key(P, &[0u8; SALT_LEN], bad), Err(Error::KdfParams)),
                "p_cost {p_cost} reached Argon2"
            );
        }
    }

    /// The other half of the same finding: `m_cost` is the number of KiB
    /// Argon2 allocates up front, so `u32::MAX` asks for 4 TiB. The allocation
    /// fails, and a failed allocation aborts the process — so simply *trying to
    /// open* a hostile file would kill the program.
    #[test]
    fn an_absurd_memory_cost_is_rejected_rather_than_allocated() {
        for m_cost in [u32::MAX, u32::MAX - 1, KdfParams::MAX_M_COST + 1] {
            let bad = KdfParams {
                m_cost,
                t_cost: 3,
                p_cost: 4,
            };
            assert!(bad.checked().is_err(), "m_cost {m_cost} accepted");
            assert!(
                matches!(derive_key(P, &[0u8; SALT_LEN], bad), Err(Error::KdfParams)),
                "m_cost {m_cost} reached Argon2"
            );
        }
        // The ceiling itself is allowed, and comfortably exceeds both RFC
        // 9106 recommendations.
        assert!(KdfParams {
            m_cost: KdfParams::MAX_M_COST,
            t_cost: 1,
            p_cost: 4
        }
        .checked()
        .is_ok());
    }

    /// Checked at compile time: the ceiling must never be tightened below the
    /// largest profile RFC 9106 actually recommends, or the cap would start
    /// refusing legitimate files rather than absurd ones.
    const _: () = assert!(KdfParams::MAX_M_COST >= 2 * 1024 * 1024);

    #[test]
    fn sane_parameters_still_pass_the_check() {
        KdfParams::default().checked().unwrap();
        KdfParams::weak_for_tests().checked().unwrap();
        assert!(KdfParams {
            m_cost: 8,
            t_cost: 1,
            p_cost: 1
        }
        .checked()
        .is_ok());
        // m_cost must still cover 8 blocks per lane.
        assert!(KdfParams {
            m_cost: 8,
            t_cost: 1,
            p_cost: 2
        }
        .checked()
        .is_err());
    }

    #[test]
    fn short_salt_is_rejected() {
        assert!(matches!(
            derive_key(P, &[0u8; 4], weak()),
            Err(Error::KdfParams)
        ));
    }

    #[test]
    fn random_salts_differ() {
        assert_ne!(random_salt().unwrap(), random_salt().unwrap());
    }
}
