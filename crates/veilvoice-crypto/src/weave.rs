// SPDX-License-Identifier: GPL-3.0-or-later
//! Twenty-seven reversible encodings, chosen at random, applied underneath the
//! encryption.
//!
//! # What this buys, and it is not what it looks like
//!
//! Say the disappointing part first, because the alternative is letting a
//! reader assume it.
//!
//! **This adds no cryptographic strength.** Every record here is sealed with
//! ChaCha20-Poly1305, whose output is already indistinguishable from random to
//! anybody without the key. Encoding the plaintext before encrypting it does
//! not make that ciphertext harder to break, and anybody who tells you a layer
//! of base91 under an AEAD is "double encryption" is wrong. If the only thing
//! standing between an attacker and your data were the encoding, the answer
//! would be: that is not security, it is a puzzle.
//!
//! So the honest list of what it does buy, all of it smaller than the previous
//! paragraph is big:
//!
//! - **Plaintext that leaks by a route other than the AEAD is not readable.**
//!   A core dump, a swap file, a page that was written before the seal, a
//!   future bug in this crate's own framing -- any of those hands somebody the
//!   plaintext buffer. `frame_ms = 4.25` in that buffer is a sentence. The
//!   same record base91-ed under a move-to-front transform is not, and cannot
//!   be grepped for.
//! - **After a key compromise there is one more step.** Small, and worth
//!   naming as small: somebody with the passphrase reads the encoding marker
//!   in the first byte and undoes it. It costs them a minute, not a month.
//! - **A partially-recovered record does not read as text.** Truncated or
//!   damaged plaintext that decodes to nothing is better than truncated
//!   plaintext that decodes to half your settings.
//!
//! That is the whole claim. It is defence in depth against exposure that does
//! not go through the cipher, not a second cipher.
//!
//! # Names are different, and the difference matters
//!
//! A record's filename is 18 bytes of HMAC, base64url-encoded to exactly 24
//! characters. Weaving those bytes first is fine -- but **only with a codec
//! that preserves length**.
//!
//! If a name could be hex-encoded it would come out 48 characters instead of
//! 24, and the length of the filename would announce which encoding was used.
//! Worse, decoys are random bytes with a random weave while records have a
//! key-derived one, so a length difference would separate the two at a glance
//! and undo the entire point of the decoys.
//!
//! So names use [`LENGTH_PRESERVING`] only, and the choice is derived from the
//! key rather than drawn at random, because a name has to be computable again
//! next time. Contents may use anything, because contents are padded to fixed
//! buckets afterwards.
//!
//! One honest consequence of that padding: an expanding codec can push a
//! record into a larger bucket than a compact one would, so writing the same
//! data twice can produce two different file sizes. That reveals nothing about
//! the data -- only that the encoding changed -- and it is the reason bucket
//! sizes are coarse.
//!
//! # In plain words
//!
//! Before VeilVoice encrypts one of its own files, it scrambles the contents
//! into one of twenty-seven odd formats picked at random, and does something
//! similar to the filename.
//!
//! It is not what keeps the file secret. The encryption does that. This means
//! that if the unencrypted contents ever escape some other way -- a crash
//! dump, a swap file -- what escapes does not read as anything.

use crate::Error;

/// Every encoding, by name.
///
/// The identity is deliberately in the set. A scheme that never leaves data
/// alone is a scheme in which "unencoded" is itself a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weave {
    // --- length preserving: safe for filenames -------------------------
    /// Left exactly as it is.
    None,
    /// Every byte moved along the alphabet by a fixed amount.
    Rotate(u8),
    /// Exclusive-or against a counter, so equal bytes stop looking equal.
    XorCounter,
    /// Every byte's bits reversed, most significant to least.
    BitReverse,
    /// The two halves of each byte exchanged.
    NibbleSwap,
    /// Reflected binary, where consecutive values differ in one bit.
    Gray,
    /// The whole sequence back to front.
    Reverse,
    /// Each byte replaced by its difference from the one before it.
    Delta,
    /// Move to front, which turns repetition into small numbers.
    MoveToFront,
    /// Every bit flipped.
    Complement,
    /// The two halves of the sequence interleaved, as in a riffle shuffle.
    Riffle,
    /// A fixed substitution over all 256 values.
    Substitute,

    // --- expanding: contents only --------------------------------------
    /// Base16, which is to say hexadecimal.
    Hex,
    /// Base32 as RFC 4648 defines it.
    Base32,
    /// Base32 with the extended hex alphabet, which sorts in value order.
    Base32Hex,
    /// Zooko's base32, whose alphabet avoids letters people mistype.
    ZBase32,
    /// Crockford's base32, which excludes I, L, O and U.
    Crockford32,
    /// Base45, as used by the European digital covid certificates.
    Base45,
    /// Ascii85 in Adobe's spelling.
    Ascii85,
    /// Z85, the ZeroMQ variant with a filename-safe alphabet.
    Z85,
    /// basE91, which packs more per character than base85.
    Base91,
    /// The six-bit-plus-space mapping at the heart of uuencode.
    Uu,
    /// The same idea over xxencode's alphabet.
    Xx,
    /// Quoted-printable, as in mail bodies.
    QuotedPrintable,
    /// Percent-encoding, as in a URL.
    Percent,
    /// yEnc, from the binary newsgroups.
    YEnc,
    /// Run-length encoding.
    RunLength,
}

/// Every encoding that leaves the byte count alone.
///
/// The only ones a filename may use. See the module note for why.
pub const LENGTH_PRESERVING: &[Weave] = &[
    Weave::None,
    // A real shift, not zero: `Rotate(0)` is the identity in disguise, and a
    // list containing two identities is a list that picks one twice as often.
    Weave::Rotate(0x5b),
    Weave::XorCounter,
    Weave::BitReverse,
    Weave::NibbleSwap,
    Weave::Gray,
    Weave::Reverse,
    Weave::Delta,
    Weave::MoveToFront,
    Weave::Complement,
    Weave::Riffle,
    Weave::Substitute,
];

/// Every encoding, for contents.
pub const ALL: &[Weave] = &[
    Weave::None,
    Weave::Rotate(0x5b),
    Weave::XorCounter,
    Weave::BitReverse,
    Weave::NibbleSwap,
    Weave::Gray,
    Weave::Reverse,
    Weave::Delta,
    Weave::MoveToFront,
    Weave::Complement,
    Weave::Riffle,
    Weave::Substitute,
    Weave::Hex,
    Weave::Base32,
    Weave::Base32Hex,
    Weave::ZBase32,
    Weave::Crockford32,
    Weave::Base45,
    Weave::Ascii85,
    Weave::Z85,
    Weave::Base91,
    Weave::Uu,
    Weave::Xx,
    Weave::QuotedPrintable,
    Weave::Percent,
    Weave::YEnc,
    Weave::RunLength,
];

impl Weave {
    /// The marker stored with a record so the encoding can be undone.
    ///
    /// `Rotate` carries its shift in the low byte of a two-byte marker; every
    /// other encoding is one byte and a zero.
    pub fn id(self) -> [u8; 2] {
        match self {
            Self::None => [0, 0],
            Self::Rotate(n) => [1, n],
            Self::XorCounter => [2, 0],
            Self::BitReverse => [3, 0],
            Self::NibbleSwap => [4, 0],
            Self::Gray => [5, 0],
            Self::Reverse => [6, 0],
            Self::Delta => [7, 0],
            Self::MoveToFront => [8, 0],
            Self::Complement => [9, 0],
            Self::Riffle => [10, 0],
            Self::Substitute => [11, 0],
            Self::Hex => [12, 0],
            Self::Base32 => [13, 0],
            Self::Base32Hex => [14, 0],
            Self::ZBase32 => [15, 0],
            Self::Crockford32 => [16, 0],
            Self::Base45 => [17, 0],
            Self::Ascii85 => [18, 0],
            Self::Z85 => [19, 0],
            Self::Base91 => [20, 0],
            Self::Uu => [21, 0],
            Self::Xx => [22, 0],
            Self::QuotedPrintable => [23, 0],
            Self::Percent => [24, 0],
            Self::YEnc => [25, 0],
            Self::RunLength => [26, 0],
        }
    }

    /// Recover an encoding from its marker.
    pub fn from_id(id: [u8; 2]) -> Result<Self, Error> {
        Ok(match id[0] {
            0 => Self::None,
            1 => Self::Rotate(id[1]),
            2 => Self::XorCounter,
            3 => Self::BitReverse,
            4 => Self::NibbleSwap,
            5 => Self::Gray,
            6 => Self::Reverse,
            7 => Self::Delta,
            8 => Self::MoveToFront,
            9 => Self::Complement,
            10 => Self::Riffle,
            11 => Self::Substitute,
            12 => Self::Hex,
            13 => Self::Base32,
            14 => Self::Base32Hex,
            15 => Self::ZBase32,
            16 => Self::Crockford32,
            17 => Self::Base45,
            18 => Self::Ascii85,
            19 => Self::Z85,
            20 => Self::Base91,
            21 => Self::Uu,
            22 => Self::Xx,
            23 => Self::QuotedPrintable,
            24 => Self::Percent,
            25 => Self::YEnc,
            26 => Self::RunLength,
            _ => return Err(Error::BadHeader),
        })
    }

    /// Whether this leaves the byte count untouched.
    pub fn preserves_length(self) -> bool {
        matches!(
            self,
            Self::None
                | Self::Rotate(_)
                | Self::XorCounter
                | Self::BitReverse
                | Self::NibbleSwap
                | Self::Gray
                | Self::Reverse
                | Self::Delta
                | Self::MoveToFront
                | Self::Complement
                | Self::Riffle
                | Self::Substitute
        )
    }

    /// Pick one at random, from `ALL`.
    pub fn random() -> Result<Self, Error> {
        let mut pick = [0u8; 2];
        getrandom::getrandom(&mut pick).map_err(|_| Error::Random)?;
        Ok(match ALL[pick[0] as usize % ALL.len()] {
            Self::Rotate(_) => Self::Rotate(pick[1] | 1),
            other => other,
        })
    }

    /// Pick one deterministically from key material, from `LENGTH_PRESERVING`.
    ///
    /// Deterministic because a filename has to be computable again on the next
    /// launch, and length preserving because the name's length must not
    /// announce the choice.
    pub fn for_name(seed: &[u8]) -> Self {
        let a = seed.first().copied().unwrap_or(0);
        let b = seed.get(1).copied().unwrap_or(0);
        match LENGTH_PRESERVING[a as usize % LENGTH_PRESERVING.len()] {
            Self::Rotate(_) => Self::Rotate(b | 1),
            other => other,
        }
    }

    /// Encode.
    pub fn apply(self, input: &[u8]) -> Vec<u8> {
        match self {
            Self::None => input.to_vec(),
            Self::Rotate(n) => input.iter().map(|b| b.wrapping_add(n)).collect(),
            Self::XorCounter => input
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ (i as u8).wrapping_mul(31).wrapping_add(7))
                .collect(),
            Self::BitReverse => input.iter().map(|b| b.reverse_bits()).collect(),
            Self::NibbleSwap => input.iter().map(|b| b.rotate_left(4)).collect(),
            Self::Gray => input.iter().map(|b| b ^ (b >> 1)).collect(),
            Self::Reverse => input.iter().rev().copied().collect(),
            Self::Delta => {
                let mut out = Vec::with_capacity(input.len());
                let mut last = 0u8;
                for b in input {
                    out.push(b.wrapping_sub(last));
                    last = *b;
                }
                out
            }
            Self::MoveToFront => {
                let mut table: Vec<u8> = (0..=255).collect();
                let mut out = Vec::with_capacity(input.len());
                for b in input {
                    let at = table.iter().position(|x| x == b).unwrap_or(0);
                    out.push(at as u8);
                    let v = table.remove(at);
                    table.insert(0, v);
                }
                out
            }
            Self::Complement => input.iter().map(|b| !b).collect(),
            Self::Riffle => {
                let half = input.len().div_ceil(2);
                let (a, b) = input.split_at(half);
                let mut out = Vec::with_capacity(input.len());
                for (i, first) in a.iter().enumerate() {
                    out.push(*first);
                    if let Some(second) = b.get(i) {
                        out.push(*second);
                    }
                }
                out
            }
            Self::Substitute => input.iter().map(|b| SBOX[*b as usize]).collect(),
            Self::Hex => {
                let mut out = Vec::with_capacity(input.len() * 2);
                for b in input {
                    out.push(HEX[(b >> 4) as usize]);
                    out.push(HEX[(b & 15) as usize]);
                }
                out
            }
            Self::Base32 => base32_encode(input, B32),
            Self::Base32Hex => base32_encode(input, B32HEX),
            Self::ZBase32 => base32_encode(input, ZB32),
            Self::Crockford32 => base32_encode(input, CROCKFORD),
            Self::Base45 => base45_encode(input),
            Self::Ascii85 => base85_encode(input, A85, b'!'),
            Self::Z85 => base85_encode(input, Z85A, 0),
            Self::Base91 => base91_encode(input),
            Self::Uu => sixbit_encode(input, UU),
            Self::Xx => sixbit_encode(input, XX),
            Self::QuotedPrintable => {
                let mut out = Vec::new();
                for b in input {
                    if b.is_ascii_alphanumeric() && *b != b'=' {
                        out.push(*b);
                    } else {
                        out.push(b'=');
                        out.push(HEX_UPPER[(b >> 4) as usize]);
                        out.push(HEX_UPPER[(b & 15) as usize]);
                    }
                }
                out
            }
            Self::Percent => {
                let mut out = Vec::new();
                for b in input {
                    if b.is_ascii_alphanumeric() {
                        out.push(*b);
                    } else {
                        out.push(b'%');
                        out.push(HEX_UPPER[(b >> 4) as usize]);
                        out.push(HEX_UPPER[(b & 15) as usize]);
                    }
                }
                out
            }
            Self::YEnc => {
                let mut out = Vec::new();
                for b in input {
                    let v = b.wrapping_add(42);
                    if matches!(v, 0x00 | 0x0A | 0x0D | 0x3D) {
                        out.push(b'=');
                        out.push(v.wrapping_add(64));
                    } else {
                        out.push(v);
                    }
                }
                out
            }
            Self::RunLength => {
                // A literal run is length-prefixed with the high bit clear; a
                // repeat with it set. Both are capped at 127 so the prefix
                // always fits, which is what makes this reversible on data
                // that does not repeat at all.
                let mut out = Vec::new();
                let mut i = 0;
                while i < input.len() {
                    let b = input[i];
                    let mut run = 1;
                    while i + run < input.len() && input[i + run] == b && run < 127 {
                        run += 1;
                    }
                    if run >= 2 {
                        out.push(0x80 | run as u8);
                        out.push(b);
                        i += run;
                    } else {
                        let start = i;
                        let mut lit = 0;
                        while i < input.len() && lit < 127 {
                            let same = i + 1 < input.len() && input[i + 1] == input[i];
                            if same && lit > 0 {
                                break;
                            }
                            if same {
                                break;
                            }
                            i += 1;
                            lit += 1;
                        }
                        if lit == 0 {
                            i = start + 1;
                            lit = 1;
                        }
                        out.push(lit as u8);
                        out.extend_from_slice(&input[start..start + lit]);
                    }
                }
                out
            }
        }
    }

    /// Decode. Any input this encoding could not have produced is refused.
    pub fn undo(self, input: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::None => input.to_vec(),
            Self::Rotate(n) => input.iter().map(|b| b.wrapping_sub(n)).collect(),
            Self::XorCounter => Self::XorCounter.apply(input),
            Self::BitReverse => input.iter().map(|b| b.reverse_bits()).collect(),
            Self::NibbleSwap => input.iter().map(|b| b.rotate_right(4)).collect(),
            Self::Gray => input
                .iter()
                .map(|b| {
                    let mut v = *b;
                    v ^= v >> 1;
                    v ^= v >> 2;
                    v ^= v >> 4;
                    v
                })
                .collect(),
            Self::Reverse => input.iter().rev().copied().collect(),
            Self::Delta => {
                let mut out = Vec::with_capacity(input.len());
                let mut last = 0u8;
                for b in input {
                    last = last.wrapping_add(*b);
                    out.push(last);
                }
                out
            }
            Self::MoveToFront => {
                let mut table: Vec<u8> = (0..=255).collect();
                let mut out = Vec::with_capacity(input.len());
                for b in input {
                    let at = *b as usize;
                    if at >= table.len() {
                        return Err(Error::BadHeader);
                    }
                    let v = table.remove(at);
                    out.push(v);
                    table.insert(0, v);
                }
                out
            }
            Self::Complement => input.iter().map(|b| !b).collect(),
            Self::Riffle => {
                let half = input.len().div_ceil(2);
                let mut a = Vec::with_capacity(half);
                let mut b = Vec::with_capacity(input.len() - half);
                for (i, v) in input.iter().enumerate() {
                    if i % 2 == 0 {
                        a.push(*v);
                    } else {
                        b.push(*v);
                    }
                }
                a.extend_from_slice(&b);
                a
            }
            Self::Substitute => input.iter().map(|b| UNSBOX[*b as usize]).collect(),
            Self::Hex => {
                if !input.len().is_multiple_of(2) {
                    return Err(Error::BadHeader);
                }
                let mut out = Vec::with_capacity(input.len() / 2);
                for pair in input.chunks(2) {
                    out.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
                }
                out
            }
            Self::Base32 => base32_decode(input, B32)?,
            Self::Base32Hex => base32_decode(input, B32HEX)?,
            Self::ZBase32 => base32_decode(input, ZB32)?,
            Self::Crockford32 => base32_decode(input, CROCKFORD)?,
            Self::Base45 => base45_decode(input)?,
            Self::Ascii85 => base85_decode(input, A85, b'!')?,
            Self::Z85 => base85_decode(input, Z85A, 0)?,
            Self::Base91 => base91_decode(input)?,
            Self::Uu => sixbit_decode(input, UU)?,
            Self::Xx => sixbit_decode(input, XX)?,
            Self::QuotedPrintable | Self::Percent => {
                let marker = if self == Self::Percent { b'%' } else { b'=' };
                let mut out = Vec::new();
                let mut i = 0;
                while i < input.len() {
                    if input[i] == marker {
                        if i + 2 >= input.len() {
                            return Err(Error::BadHeader);
                        }
                        out.push((nibble(input[i + 1])? << 4) | nibble(input[i + 2])?);
                        i += 3;
                    } else {
                        out.push(input[i]);
                        i += 1;
                    }
                }
                out
            }
            Self::YEnc => {
                let mut out = Vec::new();
                let mut i = 0;
                while i < input.len() {
                    let v = if input[i] == b'=' {
                        if i + 1 >= input.len() {
                            return Err(Error::BadHeader);
                        }
                        i += 2;
                        input[i - 1].wrapping_sub(64)
                    } else {
                        i += 1;
                        input[i - 1]
                    };
                    out.push(v.wrapping_sub(42));
                }
                out
            }
            Self::RunLength => {
                let mut out = Vec::new();
                let mut i = 0;
                while i < input.len() {
                    let head = input[i];
                    i += 1;
                    if head & 0x80 != 0 {
                        let run = (head & 0x7f) as usize;
                        if i >= input.len() {
                            return Err(Error::BadHeader);
                        }
                        out.extend(std::iter::repeat_n(input[i], run));
                        i += 1;
                    } else {
                        let run = head as usize;
                        if i + run > input.len() {
                            return Err(Error::BadHeader);
                        }
                        out.extend_from_slice(&input[i..i + run]);
                        i += run;
                    }
                }
                out
            }
        })
    }
}

/// Encode with a randomly chosen encoding, returning it so it can be undone.
///
/// # Why there are two layers and not one
///
/// The chosen encoding is applied over a fixed scramble rather than over the
/// plaintext, and that is not decoration.
///
/// Two of the twenty-seven pass some bytes through untouched. Run-length
/// copies any run that does not repeat, so `frame_ms = 4.25` survives it
/// almost intact; and `Rotate(0)`, if it were ever chosen, is the identity
/// wearing a hat. A scheme that picks uniformly at random is only as good as
/// its worst outcome, so roughly one record in twenty-seven would have been
/// left plainly readable in any buffer that escaped -- which is the single
/// thing this layer exists to prevent.
///
/// Putting an unconditional [`Weave::Substitute`] underneath fixes it for
/// every choice at once, including the identity. It is a fixed public
/// permutation and adds no secrecy whatsoever; what it adds is that **no
/// choice in the set can leave recognisable text**, which is a property of the
/// scheme rather than of a lucky draw.
pub fn encode(input: &[u8]) -> Result<(Weave, Vec<u8>), Error> {
    let chosen = Weave::random()?;
    Ok((chosen, chosen.apply(&Weave::Substitute.apply(input))))
}

/// Undo [`encode`].
pub fn decode(chosen: Weave, input: &[u8]) -> Result<Vec<u8>, Error> {
    Weave::Substitute.undo(&chosen.undo(input)?)
}

fn nibble(c: u8) -> Result<u8, Error> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(Error::BadHeader),
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";
const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const B32HEX: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
const ZB32: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const B45: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
const A85: u8 = 0;
const Z85A: u8 = 1;
const UU: u8 = 0;
const XX: u8 = 1;
const XX_ALPHABET: &[u8; 64] = b"+-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const Z85_ALPHABET: &[u8; 85] =
    b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-:+=^!/*?&<>()[]{}@%$#";

/// A fixed permutation of every byte value, and its inverse.
///
/// Generated once from a multiplicative step over the odd residues, so it is a
/// genuine bijection rather than a table somebody typed and hoped about.
const SBOX: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = ((i as u32 * 167 + 13) % 256) as u8;
        i += 1;
    }
    table
};

const UNSBOX: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        table[SBOX[i] as usize] = i as u8;
        i += 1;
    }
    table
};

fn base32_encode(input: &[u8], alphabet: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len().div_ceil(5) * 8);
    for chunk in input.chunks(5) {
        let mut buf = [0u8; 5];
        buf[..chunk.len()].copy_from_slice(chunk);
        let n = u64::from_be_bytes([0, 0, 0, buf[0], buf[1], buf[2], buf[3], buf[4]]);
        let characters = match chunk.len() {
            1 => 2,
            2 => 4,
            3 => 5,
            4 => 7,
            _ => 8,
        };
        for i in 0..characters {
            out.push(alphabet[((n >> (35 - i * 5)) & 31) as usize]);
        }
    }
    out
}

fn base32_decode(input: &[u8], alphabet: &[u8; 32]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    for chunk in input.chunks(8) {
        let mut n = 0u64;
        for (i, c) in chunk.iter().enumerate() {
            let v = alphabet
                .iter()
                .position(|a| a == c)
                .ok_or(Error::BadHeader)? as u64;
            n |= v << (35 - i * 5);
        }
        let bytes = match chunk.len() {
            2 => 1,
            4 => 2,
            5 => 3,
            7 => 4,
            8 => 5,
            _ => return Err(Error::BadHeader),
        };
        let be = n.to_be_bytes();
        out.extend_from_slice(&be[3..3 + bytes]);
    }
    Ok(out)
}

fn base45_encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for pair in input.chunks(2) {
        if pair.len() == 2 {
            let n = u16::from_be_bytes([pair[0], pair[1]]) as u32;
            out.push(B45[(n % 45) as usize]);
            out.push(B45[((n / 45) % 45) as usize]);
            out.push(B45[(n / 2025) as usize]);
        } else {
            let n = pair[0] as u32;
            out.push(B45[(n % 45) as usize]);
            out.push(B45[(n / 45) as usize]);
        }
    }
    out
}

fn base45_decode(input: &[u8]) -> Result<Vec<u8>, Error> {
    let value = |c: &u8| {
        B45.iter()
            .position(|a| a == c)
            .map(|v| v as u32)
            .ok_or(Error::BadHeader)
    };
    let mut out = Vec::new();
    for chunk in input.chunks(3) {
        match chunk.len() {
            3 => {
                let n = value(&chunk[0])? + value(&chunk[1])? * 45 + value(&chunk[2])? * 2025;
                let n = u16::try_from(n).map_err(|_| Error::BadHeader)?;
                out.extend_from_slice(&n.to_be_bytes());
            }
            2 => {
                let n = value(&chunk[0])? + value(&chunk[1])? * 45;
                out.push(u8::try_from(n).map_err(|_| Error::BadHeader)?);
            }
            _ => return Err(Error::BadHeader),
        }
    }
    Ok(out)
}

fn base85_encode(input: &[u8], flavour: u8, offset: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in input.chunks(4) {
        let mut buf = [0u8; 4];
        buf[..chunk.len()].copy_from_slice(chunk);
        let mut n = u32::from_be_bytes(buf);
        let mut group = [0u8; 5];
        for slot in group.iter_mut().rev() {
            let digit = (n % 85) as usize;
            n /= 85;
            *slot = if flavour == Z85A {
                Z85_ALPHABET[digit]
            } else {
                offset + digit as u8
            };
        }
        out.extend_from_slice(&group[..chunk.len() + 1]);
    }
    out
}

fn base85_decode(input: &[u8], flavour: u8, offset: u8) -> Result<Vec<u8>, Error> {
    let value = |c: u8| -> Result<u32, Error> {
        if flavour == Z85A {
            Z85_ALPHABET
                .iter()
                .position(|a| *a == c)
                .map(|v| v as u32)
                .ok_or(Error::BadHeader)
        } else {
            let v = c.checked_sub(offset).ok_or(Error::BadHeader)?;
            if v >= 85 {
                return Err(Error::BadHeader);
            }
            Ok(v as u32)
        }
    };
    let mut out = Vec::new();
    for chunk in input.chunks(5) {
        if chunk.len() < 2 {
            return Err(Error::BadHeader);
        }
        let mut n: u32 = 0;
        for i in 0..5 {
            let digit = if i < chunk.len() {
                value(chunk[i])?
            } else {
                84
            };
            n = n.checked_mul(85).ok_or(Error::BadHeader)?;
            n = n.checked_add(digit).ok_or(Error::BadHeader)?;
        }
        let be = n.to_be_bytes();
        out.extend_from_slice(&be[..chunk.len() - 1]);
    }
    Ok(out)
}

const B91: &[u8; 91] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~\"";

fn base91_encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut queue, mut bits) = (0u32, 0u32);
    for byte in input {
        queue |= (*byte as u32) << bits;
        bits += 8;
        if bits > 13 {
            let mut value = queue & 8191;
            if value > 88 {
                queue >>= 13;
                bits -= 13;
            } else {
                value = queue & 16383;
                queue >>= 14;
                bits -= 14;
            }
            out.push(B91[(value % 91) as usize]);
            out.push(B91[(value / 91) as usize]);
        }
    }
    if bits > 0 {
        out.push(B91[(queue % 91) as usize]);
        if bits > 7 || queue > 90 {
            out.push(B91[(queue / 91) as usize]);
        }
    }
    out
}

fn base91_decode(input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let (mut queue, mut bits) = (0u32, 0u32);
    let mut pending: i32 = -1;
    for c in input {
        let digit = B91.iter().position(|a| a == c).ok_or(Error::BadHeader)? as i32;
        if pending < 0 {
            pending = digit;
            continue;
        }
        let value = (pending + digit * 91) as u32;
        queue |= value << bits;
        bits += if value & 8191 > 88 { 13 } else { 14 };
        while bits > 7 {
            out.push((queue & 255) as u8);
            queue >>= 8;
            bits -= 8;
        }
        pending = -1;
    }
    if pending >= 0 {
        out.push(((queue | (pending as u32) << bits) & 255) as u8);
    }
    Ok(out)
}

fn sixbit_encode(input: &[u8], flavour: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in input.chunks(3) {
        let mut buf = [0u8; 3];
        buf[..chunk.len()].copy_from_slice(chunk);
        let n = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | buf[2] as u32;
        for i in 0..chunk.len() + 1 {
            let six = ((n >> (18 - i * 6)) & 63) as usize;
            out.push(if flavour == XX {
                XX_ALPHABET[six]
            } else if six == 0 {
                b'`'
            } else {
                six as u8 + 32
            });
        }
    }
    out
}

fn sixbit_decode(input: &[u8], flavour: u8) -> Result<Vec<u8>, Error> {
    let value = |c: u8| -> Result<u32, Error> {
        if flavour == XX {
            XX_ALPHABET
                .iter()
                .position(|a| *a == c)
                .map(|v| v as u32)
                .ok_or(Error::BadHeader)
        } else if c == b'`' {
            Ok(0)
        } else {
            let v = c.checked_sub(32).ok_or(Error::BadHeader)?;
            if v >= 64 {
                return Err(Error::BadHeader);
            }
            Ok(v as u32)
        }
    };
    let mut out = Vec::new();
    for chunk in input.chunks(4) {
        if chunk.len() < 2 {
            return Err(Error::BadHeader);
        }
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - i * 6);
        }
        let bytes = chunk.len() - 1;
        let be = n.to_be_bytes();
        out.extend_from_slice(&be[1..1 + bytes]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every input worth trying, including the ones that break naive codecs.
    fn corpus() -> Vec<Vec<u8>> {
        let mut cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![0],
            vec![255],
            vec![0, 0, 0, 0, 0, 0, 0, 0],
            vec![255; 17],
            b"frame_ms = 4.25\nspeed = 98.50\nsessions = 3\n".to_vec(),
            (0..=255u8).collect(),
            (0..=255u8).rev().collect(),
            vec![0x41; 300],
        ];
        // Every length from 0 to 40, so no codec's block edge goes untried.
        for n in 0..=40usize {
            cases.push((0..n).map(|i| (i * 37 + 11) as u8).collect());
        }
        // A little pseudo-random data, deterministic so a failure is
        // reproducible rather than a story about a run that once happened.
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for len in [1, 7, 64, 255, 1024] {
            let mut v = Vec::with_capacity(len);
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                v.push((state & 0xff) as u8);
            }
            cases.push(v);
        }
        cases
    }

    #[test]
    fn every_encoding_round_trips_every_input() {
        for weave in ALL {
            let weave = match weave {
                Weave::Rotate(_) => Weave::Rotate(137),
                other => *other,
            };
            for input in corpus() {
                let encoded = weave.apply(&input);
                let decoded = weave.undo(&encoded).unwrap_or_else(|e| {
                    panic!(
                        "{weave:?} could not undo its own output for {} bytes: {e}",
                        input.len()
                    )
                });
                assert_eq!(
                    decoded,
                    input,
                    "{weave:?} did not round trip {} bytes",
                    input.len()
                );
            }
        }
    }

    #[test]
    fn there_are_at_least_twenty_of_them() {
        assert!(ALL.len() >= 20, "only {} encodings", ALL.len());
        assert_eq!(ALL.len(), 27);
    }

    #[test]
    fn every_marker_is_distinct_and_recoverable() {
        let mut seen = std::collections::BTreeSet::new();
        for weave in ALL {
            let id = weave.id();
            assert!(seen.insert(id[0]), "two encodings share marker {}", id[0]);
            assert_eq!(
                Weave::from_id(id).unwrap(),
                *weave,
                "{weave:?} does not come back from its own marker"
            );
        }
    }

    #[test]
    fn an_unknown_marker_is_refused_rather_than_guessed() {
        assert!(Weave::from_id([200, 0]).is_err());
    }

    #[test]
    fn a_name_encoding_never_changes_the_length() {
        // The whole reason names use a restricted set: a filename whose length
        // varies announces which encoding produced it, and separates records
        // from decoys at a glance.
        for weave in LENGTH_PRESERVING {
            let weave = match weave {
                Weave::Rotate(_) => Weave::Rotate(99),
                other => *other,
            };
            assert!(weave.preserves_length(), "{weave:?} is in the wrong list");
            for input in corpus() {
                assert_eq!(
                    weave.apply(&input).len(),
                    input.len(),
                    "{weave:?} changed the length of {} bytes",
                    input.len()
                );
            }
        }
    }

    #[test]
    fn the_name_choice_is_stable_for_one_seed() {
        // Names are derived, so an unstable choice would lose every record on
        // the next launch.
        for seed in [&[0u8, 0][..], &[7, 200], &[255, 1], &[]] {
            assert_eq!(Weave::for_name(seed), Weave::for_name(seed));
        }
    }

    #[test]
    fn different_seeds_reach_different_encodings() {
        let picked: std::collections::BTreeSet<_> = (0u8..=255)
            .map(|i| format!("{:?}", Weave::for_name(&[i, i])))
            .collect();
        assert!(
            picked.len() >= LENGTH_PRESERVING.len(),
            "only {} of {} name encodings are reachable",
            picked.len(),
            LENGTH_PRESERVING.len()
        );
    }

    #[test]
    fn random_reaches_every_encoding_eventually() {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..4000 {
            seen.insert(Weave::random().unwrap().id()[0]);
        }
        assert_eq!(
            seen.len(),
            ALL.len(),
            "only {} of {} encodings were ever chosen",
            seen.len(),
            ALL.len()
        );
    }

    #[test]
    fn the_substitution_is_a_real_bijection() {
        let distinct: std::collections::BTreeSet<u8> = SBOX.iter().copied().collect();
        assert_eq!(distinct.len(), 256, "the S-box loses values");
        for i in 0..=255u8 {
            assert_eq!(UNSBOX[SBOX[i as usize] as usize], i);
        }
    }

    /// The one thing this layer actually buys, checked for **every** choice.
    ///
    /// The first version of this tested `Weave::apply` directly and found two
    /// encodings that leave plaintext readable: run-length copies any run that
    /// does not repeat, so `frame_ms = 4.25` came through it almost intact,
    /// and `Rotate(0)` was the identity by accident. A scheme that picks
    /// uniformly at random is only as good as its worst outcome, so one record
    /// in twenty-seven would have been left greppable in any buffer that
    /// escaped.
    ///
    /// That is why `encode` puts a fixed substitution underneath the choice,
    /// and why this test goes through `encode` rather than `apply`.
    #[test]
    fn no_choice_leaves_recognisable_text() {
        let secret = b"frame_ms = 4.25\nsessions = 3\npassphrase = hunter2\n";
        for weave in ALL {
            let weave = match weave {
                Weave::Rotate(_) => Weave::Rotate(0x5b),
                other => *other,
            };
            let woven = weave.apply(&Weave::Substitute.apply(secret));
            for needle in [
                &b"frame_ms"[..],
                &b"passphrase"[..],
                &b"sessions"[..],
                &b"hunter2"[..],
            ] {
                assert!(
                    !woven.windows(needle.len()).any(|w| w == needle),
                    "{weave:?} left {:?} readable in its output",
                    String::from_utf8_lossy(needle)
                );
            }
        }
    }

    #[test]
    fn the_layered_pair_round_trips() {
        for input in corpus() {
            let (chosen, encoded) = encode(&input).unwrap();
            assert_eq!(
                decode(chosen, &encoded).unwrap(),
                input,
                "{chosen:?} did not survive the round trip through encode"
            );
        }
    }

    #[test]
    fn the_list_holds_no_second_identity() {
        // `Rotate(0)` is the identity wearing a hat, and a list with two
        // identities in it picks one twice as often as anything else.
        for weave in ALL {
            assert_ne!(*weave, Weave::Rotate(0), "Rotate(0) is Weave::None");
        }
    }

    #[test]
    fn decoding_rubbish_fails_rather_than_returning_something() {
        // Not every codec can reject every input -- a byte permutation accepts
        // anything by construction, and says so through `preserves_length`.
        // The ones with an alphabet must refuse bytes outside it.
        let rubbish = vec![0xff_u8; 32];
        for weave in ALL {
            // A byte permutation accepts anything by construction, and says so
            // through `preserves_length`. yEnc and run-length are framings
            // rather than alphabets, and quoted-printable and percent-encoding
            // pass unrecognised bytes through verbatim, which is what those
            // formats do rather than a defect in them.
            if weave.preserves_length()
                || matches!(
                    weave,
                    Weave::YEnc | Weave::RunLength | Weave::QuotedPrintable | Weave::Percent
                )
            {
                continue;
            }
            assert!(
                weave.undo(&rubbish).is_err(),
                "{weave:?} accepted 32 bytes of 0xff as its own output"
            );
        }
    }
}
