//! Effective bandwidth / quality hint (LC §5.2).

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

const FRAME: usize = 8192;
const PERIOD: usize = 32768;
const BAND_HZ: u64 = 100;

/// Averaged power spectrum of the mono mix: one Hann-windowed 8192-sample frame
/// per 32768-sample period.
pub struct Spectrum {
    sample_rate: u32,
    pos: usize,
    frame: Vec<f32>,
    window: Vec<f32>,
    input: Vec<f32>,
    output: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    fft: Arc<dyn RealToComplex<f32>>,
    acc: Vec<f64>,
    count: u64,
}

impl Spectrum {
    pub fn new(sample_rate: u32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FRAME);
        let window = (0..FRAME)
            .map(|n| {
                let x = 2.0 * std::f64::consts::PI * n as f64 / (FRAME - 1) as f64;
                (0.5 - 0.5 * x.cos()) as f32
            })
            .collect();
        let output = fft.make_output_vec();
        Self {
            sample_rate,
            pos: 0,
            frame: vec![0.0; FRAME],
            window,
            input: fft.make_input_vec(),
            acc: vec![0.0; output.len()],
            output,
            scratch: fft.make_scratch_vec(),
            fft,
            count: 0,
        }
    }

    pub fn push(&mut self, interleaved: &[f32], channels: usize) {
        let channels = channels.max(1);
        let scale = 1.0 / channels as f32;
        for frame in interleaved.chunks_exact(channels) {
            if self.pos < FRAME {
                self.frame[self.pos] = frame.iter().sum::<f32>() * scale;
                if self.pos + 1 == FRAME {
                    self.process_frame();
                }
            }
            self.pos += 1;
            if self.pos == PERIOD {
                self.pos = 0;
            }
        }
    }

    fn process_frame(&mut self) {
        for ((dst, &s), &w) in self.input.iter_mut().zip(&self.frame).zip(&self.window) {
            *dst = s * w;
        }
        self.fft
            .process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch)
            .expect("fft buffer sizes come from the plan");
        for (a, x) in self.acc.iter_mut().zip(&self.output) {
            *a += f64::from(x.re) * f64::from(x.re) + f64::from(x.im) * f64::from(x.im);
        }
        self.count += 1;
    }

    /// `effectiveBandwidthHz`, or `None` when nothing was pushed or the 2–8 kHz
    /// reference power is below 1e-20 (silence).
    pub fn finish(mut self) -> Option<u32> {
        if self.count == 0 && self.pos > 0 {
            // Shorter than one frame: zero-pad what was collected.
            self.frame[self.pos..].fill(0.0);
            self.process_frame();
        }
        if self.count == 0 {
            return None;
        }
        let sr = u64::from(self.sample_rate);
        let count = self.count as f64;
        let frame = FRAME as u64;
        let top_band = ((sr / 2 - 1) / BAND_HZ) as usize;
        let mut band_sum = vec![0.0f64; top_band + 1];
        let mut band_n = vec![0u64; top_band + 1];
        let (mut ref_sum, mut ref_n) = (0.0f64, 0u64);
        for (k, &a) in self.acc.iter().enumerate() {
            let p = a / count;
            // Bin centre is k*sr/8192 Hz; compare k*sr against Hz*8192 to stay exact.
            let c = k as u64 * sr;
            if (2000 * frame..8000 * frame).contains(&c) {
                ref_sum += p;
                ref_n += 1;
            }
            let b = (c / (BAND_HZ * frame)) as usize;
            if b <= top_band {
                band_sum[b] += p;
                band_n[b] += 1;
            }
        }
        if ref_n == 0 {
            return None;
        }
        let ref_power = ref_sum / ref_n as f64;
        if ref_power < 1e-20 {
            return None;
        }
        let ref_db = 10.0 * ref_power.log10();
        (0..=top_band).rev().find_map(|b| {
            let mean = if band_n[b] == 0 { 0.0 } else { band_sum[b] / band_n[b] as f64 };
            let level_db = 10.0 * (mean + 1e-30).log10();
            (level_db > ref_db - 75.0).then(|| (BAND_HZ as u32 * (b as u32 + 1)).min(self.sample_rate / 2))
        })
    }
}

/// `qualityHint` for an effective bandwidth (LC §5.2 table).
pub fn quality_hint(bw: u32, sample_rate: u32) -> &'static str {
    if bw >= 19_500 || bw >= (sample_rate / 2).saturating_sub(1000) {
        "full"
    } else if bw >= 18_000 {
        "high"
    } else if bw >= 15_500 {
        "medium"
    } else {
        "low"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_thresholds() {
        assert_eq!(quality_hint(19_500, 44_100), "full");
        assert_eq!(quality_hint(15_100, 32_000), "full");
        assert_eq!(quality_hint(19_499, 44_100), "high");
        assert_eq!(quality_hint(18_000, 44_100), "high");
        assert_eq!(quality_hint(17_999, 44_100), "medium");
        assert_eq!(quality_hint(15_500, 44_100), "medium");
        assert_eq!(quality_hint(15_499, 44_100), "low");
    }
}
