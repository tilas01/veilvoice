// SPDX-License-Identifier: GPL-3.0-or-later
//! The shape of the audio, reduced to something a page can draw.
//!
//! # Peaks, not samples
//!
//! A minute of audio at 48 kHz is 2.88 million samples and a waveform is about
//! a thousand pixels wide. Drawing every sample would produce a path megabytes
//! long that renders as a solid block.
//!
//! So the audio is divided into as many buckets as there are columns, and each
//! bucket keeps its **minimum and maximum** — not its average, and not its
//! root-mean-square. The extremes are what a waveform is: they are what makes a
//! plosive look like a plosive, and an average over a bucket of a symmetric
//! waveform is approximately zero however loud it was.
//!
//! # It is drawn from the veiled audio
//!
//! Worth stating, because the alternative is an easy mistake. The picture is of
//! the **output**, not the input. A waveform is not a voiceprint — it carries
//! no formants and no phase — but it does carry timing and loudness, and a
//! picture of the original would show the original's timing and loudness beside
//! a recording that had gone to some trouble to replace them.
//!
//! # What a waveform still shows
//!
//! Silences, the rhythm of speech, how loud somebody was, and roughly where a
//! sentence ends. That is the same turn-taking structure a conversation render
//! keeps on purpose, drawn rather than heard, and it is not additional exposure
//! beyond what the audio already carries.
//!
//! # In plain words
//!
//! Turns a recording into the wavy shape you see drawn along the bottom of an
//! audio player.
//!
//! A minute of sound is nearly three million numbers, which is far more than a
//! picture a few hundred pixels wide can show or a web page should carry. So the
//! sound is divided into as many pieces as there are columns to draw, and each
//! piece is reduced to its loudest point.
//!
//! The loudest point rather than the average, because averaging smooths a
//! recording into a flat sausage and loses exactly the peaks that make a waveform
//! worth looking at.

/// The peak envelope of a signal: one minimum and one maximum per column.
#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    /// The lowest sample in each bucket, from -1.0 to 0.0.
    pub min: Vec<f32>,
    /// The highest sample in each bucket, from 0.0 to 1.0.
    pub max: Vec<f32>,
}

impl Envelope {
    /// How many columns this envelope has.
    pub fn len(&self) -> usize {
        self.min.len()
    }

    /// Whether there are no columns at all.
    pub fn is_empty(&self) -> bool {
        self.min.is_empty()
    }
}

/// Reduce `samples` to `columns` peak pairs.
///
/// `columns` of zero, or an empty signal, gives an empty envelope rather than
/// dividing by zero. A signal shorter than the column count gives every column
/// a bucket of at least one sample, so a very short file draws as a few tall
/// columns rather than as nothing.
pub fn envelope(samples: &[f32], columns: usize) -> Envelope {
    if columns == 0 || samples.is_empty() {
        return Envelope {
            min: Vec::new(),
            max: Vec::new(),
        };
    }
    let mut min = Vec::with_capacity(columns);
    let mut max = Vec::with_capacity(columns);
    for column in 0..columns {
        let from = column * samples.len() / columns;
        let to = ((column + 1) * samples.len() / columns)
            .max(from + 1)
            .min(samples.len());
        let bucket = &samples[from..to];
        let mut low = 0.0f32;
        let mut high = 0.0f32;
        for sample in bucket {
            // A non-finite sample is skipped rather than propagated: one NaN in
            // a bucket would make the whole column NaN and the path unparseable
            // by every renderer, which fails as a blank page rather than as a
            // wrong one.
            if !sample.is_finite() {
                continue;
            }
            low = low.min(*sample);
            high = high.max(*sample);
        }
        min.push(low.clamp(-1.0, 0.0));
        max.push(high.clamp(0.0, 1.0));
    }
    Envelope { min, max }
}

/// The envelope as an SVG path, filled, inside a box.
///
/// Traced left to right along the maxima and right to left along the minima,
/// then closed — one filled shape rather than a thousand rectangles, which is a
/// tenth of the markup and draws in one operation.
///
/// Coordinates are rounded to two decimals. A waveform does not need more, and
/// full `f32` precision triples the size of the path for a difference nothing
/// can display.
pub fn svg_path(envelope: &Envelope, x: f32, y: f32, width: f32, height: f32) -> String {
    if envelope.is_empty() || width <= 0.0 || height <= 0.0 {
        return String::new();
    }
    let middle = y + height / 2.0;
    let half = height / 2.0;
    let step = width / envelope.len() as f32;
    let mut path = String::with_capacity(envelope.len() * 16);

    for (column, high) in envelope.max.iter().enumerate() {
        let px = x + column as f32 * step;
        let py = middle - high * half;
        path.push_str(if column == 0 { "M" } else { "L" });
        path.push_str(&format!("{px:.2} {py:.2} "));
    }
    for (column, low) in envelope.min.iter().enumerate().rev() {
        let px = x + column as f32 * step;
        let py = middle - low * half;
        path.push_str(&format!("L{px:.2} {py:.2} "));
    }
    path.push('Z');
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (i as f32 / len as f32) * 2.0 - 1.0)
            .collect()
    }

    #[test]
    fn an_envelope_has_one_pair_per_column() {
        let envelope = envelope(&ramp(10_000), 200);
        assert_eq!(envelope.len(), 200);
        assert_eq!(envelope.max.len(), 200);
        assert!(!envelope.is_empty());
    }

    /// The extremes are the point. An average over a symmetric waveform is
    /// approximately zero however loud it was, and a waveform drawn from
    /// averages is a flat line.
    #[test]
    fn a_loud_symmetric_signal_does_not_average_away() {
        let alternating: Vec<f32> = (0..4800)
            .map(|i| if i % 2 == 0 { 0.9 } else { -0.9 })
            .collect();
        let envelope = envelope(&alternating, 100);
        for column in 0..envelope.len() {
            assert!(envelope.max[column] > 0.8, "column {column} lost its peak");
            assert!(envelope.min[column] < -0.8, "column {column} lost its peak");
        }
    }

    #[test]
    fn silence_is_flat_and_still_has_its_columns() {
        let envelope = envelope(&vec![0.0f32; 4800], 64);
        assert_eq!(envelope.len(), 64);
        assert!(envelope.max.iter().all(|v| *v == 0.0));
        assert!(envelope.min.iter().all(|v| *v == 0.0));
    }

    /// One NaN would make a column NaN and the path unparseable, which fails
    /// as a blank page rather than as a wrong one.
    #[test]
    fn a_non_finite_sample_does_not_poison_its_column() {
        let mut samples = vec![0.5f32; 1000];
        samples[10] = f32::NAN;
        samples[11] = f32::INFINITY;
        samples[12] = f32::NEG_INFINITY;
        let envelope = envelope(&samples, 10);
        assert!(envelope.max.iter().all(|v| v.is_finite()));
        assert!(envelope.min.iter().all(|v| v.is_finite()));
        assert!(!svg_path(&envelope, 0.0, 0.0, 100.0, 40.0).contains("NaN"));
    }

    /// Samples outside full scale are clamped, so the path cannot escape its
    /// box and draw over the rest of the page.
    #[test]
    fn a_signal_past_full_scale_stays_inside_its_box() {
        let envelope = envelope(&[5.0, -5.0, 3.0, -3.0], 2);
        assert!(envelope.max.iter().all(|v| *v <= 1.0));
        assert!(envelope.min.iter().all(|v| *v >= -1.0));

        let path = svg_path(&envelope, 10.0, 20.0, 100.0, 40.0);
        for pair in path.trim_end_matches('Z').split(['M', 'L']) {
            let mut parts = pair.split_whitespace();
            let (Some(px), Some(py)) = (parts.next(), parts.next()) else {
                continue;
            };
            let px: f32 = px.parse().unwrap();
            let py: f32 = py.parse().unwrap();
            assert!((10.0..=110.0).contains(&px), "{px} is outside the box");
            assert!((20.0..=60.0).contains(&py), "{py} is outside the box");
        }
    }

    #[test]
    fn nothing_in_gives_nothing_out_rather_than_dividing_by_zero() {
        assert!(envelope(&[], 100).is_empty());
        assert!(envelope(&[0.1, 0.2], 0).is_empty());
        assert!(svg_path(&envelope(&[], 100), 0.0, 0.0, 10.0, 10.0).is_empty());
        assert!(svg_path(&envelope(&[0.5], 4), 0.0, 0.0, 0.0, 10.0).is_empty());
        assert!(svg_path(&envelope(&[0.5], 4), 0.0, 0.0, 10.0, 0.0).is_empty());
    }

    /// More columns than samples must still give a column each, so a very
    /// short file draws as something rather than as nothing.
    #[test]
    fn more_columns_than_samples_still_draws() {
        let envelope = envelope(&[0.9, -0.9], 16);
        assert_eq!(envelope.len(), 16);
        assert!(envelope.max.iter().any(|v| *v > 0.5));
    }

    /// One closed shape, not a thousand rectangles.
    #[test]
    fn the_path_is_a_single_closed_shape() {
        let path = svg_path(&envelope(&ramp(1000), 50), 0.0, 0.0, 100.0, 40.0);
        assert_eq!(path.matches('M').count(), 1, "more than one subpath");
        assert!(path.ends_with('Z'));
        // Out along the maxima and back along the minima: two points per
        // column, less the one that starts with M.
        assert_eq!(path.matches('L').count(), 50 * 2 - 1);
    }

    /// Two decimals is all a waveform needs, and full precision triples the
    /// markup for a difference nothing can display.
    #[test]
    fn coordinates_are_rounded() {
        let path = svg_path(&envelope(&ramp(999), 7), 0.0, 0.0, 100.0, 41.0);
        for number in path
            .trim_end_matches('Z')
            .split(['M', 'L', ' '])
            .filter(|part| !part.is_empty())
        {
            if let Some(point) = number.split('.').nth(1) {
                assert!(point.len() <= 2, "{number} has more than two decimals");
            }
        }
    }
}
