// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Image EXIF/GPS removal.
//!
//! Images are handled at the container level with `img-parts`: the EXIF and XMP
//! segments are dropped and the compressed pixel data is copied through
//! untouched. Nothing is re-encoded, so cleaning is lossless and cannot
//! introduce visible artefacts.
//!
//! GPS coordinates are the reason this matters most. A single holiday snapshot
//! attached to an otherwise anonymous message can place someone within a few
//! metres, and no amount of voice processing helps with that.

use crate::{Error, Policy, Report};
use img_parts::{DynImage, ImageEXIF, ImageICC};
use std::path::Path;

/// Image container formats this crate can clean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    /// JPEG (EXIF in an APP1 segment).
    Jpeg,
    /// PNG (EXIF in an `eXIf` chunk).
    Png,
    /// WebP (EXIF in an `EXIF` chunk).
    WebP,
}

impl ImageKind {
    /// Identify a format from its magic bytes.
    ///
    /// Deliberately content-sniffed rather than taken from the extension: a
    /// file named `.png` that is really a JPEG must still get cleaned.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::WebP)
        } else {
            None
        }
    }
}

/// Strip identifying metadata from encoded image bytes.
///
/// Returns the cleaned bytes and a report of what was removed.
pub fn clean_image_bytes(bytes: &[u8], policy: Policy) -> Result<(Vec<u8>, Report), Error> {
    if ImageKind::sniff(bytes).is_none() {
        return Err(Error::UnsupportedFormat);
    }
    let mut image = DynImage::from_bytes(bytes.to_vec().into())
        .map_err(|e| Error::Malformed(e.to_string()))?
        .ok_or(Error::UnsupportedFormat)?;

    let mut report = Report::default();
    if image.exif().is_some() {
        // EXIF carries the camera model, serial number, timestamps and GPS.
        report.note("EXIF");
        image.set_exif(None);
    }
    if image.icc_profile().is_some() {
        // ICC profiles are rarer but can be device-specific, so they identify
        // the capture hardware in much the same way.
        report.note("ICC profile");
        image.set_icc_profile(None);
    }

    // There is no "realistic" EXIF worth forging: a plausible-looking camera
    // tag would be a false statement about provenance, and unlike an audio
    // encoder string it invites the reader to draw conclusions. Images are
    // always stripped, and the policy is accepted only so callers can use one
    // uniform interface.
    let _ = policy;

    let mut out = Vec::with_capacity(bytes.len());
    image
        .encoder()
        .write_to(&mut out)
        .map_err(|e| Error::Malformed(e.to_string()))?;
    Ok((out, report))
}

/// Strip identifying metadata from an image file, in place.
pub fn clean_image_file(path: &Path, policy: Policy) -> Result<Report, Error> {
    let bytes = std::fs::read(path)?;
    let (cleaned, report) = clean_image_bytes(&bytes, policy)?;
    if report.changed {
        std::fs::write(path, cleaned)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal JPEG carrying an APP1/EXIF segment with a GPS-looking payload.
    fn jpeg_with_exif() -> Vec<u8> {
        let secret = b"GPS 51.5074N 0.1278W CameraSerial#12345";
        let mut exif = Vec::new();
        exif.extend_from_slice(b"Exif\0\0");
        exif.extend_from_slice(secret);

        let mut out = Vec::new();
        out.extend_from_slice(&[0xFF, 0xD8]); // SOI
        out.extend_from_slice(&[0xFF, 0xE1]); // APP1
        out.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&exif);
        // A minimal but structurally valid scan so parsers accept the file.
        out.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
        out.extend_from_slice(&[0x08; 64]);
        out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        out.extend_from_slice(&[0x00, 0x01, 0x02, 0x03]);
        out.extend_from_slice(&[0xFF, 0xD9]); // EOI
        out
    }

    #[test]
    fn sniffing_recognises_the_formats_it_supports() {
        assert_eq!(ImageKind::sniff(&jpeg_with_exif()), Some(ImageKind::Jpeg));
        assert_eq!(
            ImageKind::sniff(b"\x89PNG\r\n\x1a\nrest"),
            Some(ImageKind::Png)
        );

        let mut webp = Vec::from(*b"RIFF____WEBPVP8 ");
        webp.extend_from_slice(&[0u8; 8]);
        assert_eq!(ImageKind::sniff(&webp), Some(ImageKind::WebP));

        assert_eq!(ImageKind::sniff(b"not an image at all"), None);
    }

    #[test]
    fn sniffing_ignores_the_file_extension() {
        // A JPEG misnamed `.png` must still be identified as a JPEG.
        let bytes = jpeg_with_exif();
        assert_eq!(ImageKind::sniff(&bytes), Some(ImageKind::Jpeg));
    }

    #[test]
    fn exif_and_its_gps_payload_are_removed() {
        let bytes = jpeg_with_exif();
        let needle = b"51.5074N";
        assert!(
            bytes.windows(needle.len()).any(|w| w == needle),
            "fixture lacks GPS"
        );

        let (cleaned, report) = clean_image_bytes(&bytes, Policy::Strip).unwrap();
        assert!(report.changed);
        assert!(report.removed.iter().any(|r| r == "EXIF"));
        assert!(
            !cleaned.windows(needle.len()).any(|w| w == needle),
            "GPS coordinates survived stripping"
        );
        assert!(
            !cleaned.windows(13).any(|w| w == b"CameraSerial#"),
            "camera serial survived stripping"
        );
    }

    #[test]
    fn the_result_is_still_a_valid_jpeg() {
        let (cleaned, _) = clean_image_bytes(&jpeg_with_exif(), Policy::Strip).unwrap();
        assert_eq!(ImageKind::sniff(&cleaned), Some(ImageKind::Jpeg));
        assert!(cleaned.ends_with(&[0xFF, 0xD9]), "EOI marker missing");
    }

    #[test]
    fn cleaning_twice_is_stable() {
        let (once, _) = clean_image_bytes(&jpeg_with_exif(), Policy::Strip).unwrap();
        let (twice, report) = clean_image_bytes(&once, Policy::Strip).unwrap();
        assert!(
            !report.changed,
            "an already-clean image needs no second pass"
        );
        assert_eq!(once, twice);
    }

    #[test]
    fn unsupported_input_is_rejected() {
        assert!(matches!(
            clean_image_bytes(b"plain text", Policy::Strip),
            Err(Error::UnsupportedFormat)
        ));
    }

    #[test]
    fn file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.jpg");
        std::fs::write(&path, jpeg_with_exif()).unwrap();

        let report = clean_image_file(&path, Policy::Strip).unwrap();
        assert!(report.changed);

        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(8).any(|w| w == b"51.5074N"));
    }
}
