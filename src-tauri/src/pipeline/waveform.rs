//! Waveform peaks (LC §5.3).

const BLOCK_FRAMES: usize = 1024;
const POINTS: usize = 800;

/// Streaming peak meter: per-1024-frame block peaks, reduced to 800 points.
pub struct Waveform {
    block_samples: usize,
    filled: usize,
    current: f32,
    blocks: Vec<f32>,
}

impl Waveform {
    pub fn new(channels: usize) -> Self {
        Self { block_samples: BLOCK_FRAMES * channels.max(1), filled: 0, current: 0.0, blocks: Vec::new() }
    }

    pub fn push(&mut self, interleaved: &[f32]) {
        for &s in interleaved {
            self.current = self.current.max(s.abs());
            self.filled += 1;
            if self.filled == self.block_samples {
                self.blocks.push(self.current);
                self.filled = 0;
                self.current = 0.0;
            }
        }
    }

    pub fn finish(mut self) -> Vec<f32> {
        if self.filled > 0 {
            self.blocks.push(self.current);
        }
        let n = self.blocks.len();
        if n == 0 {
            return vec![0.0; POINTS];
        }
        (0..POINTS)
            .map(|i| {
                let lo = i * n / POINTS;
                let hi = ((i + 1) * n / POINTS).max(lo + 1);
                let peak = self.blocks[lo.min(n - 1)..hi.min(n)].iter().copied().fold(0.0f32, f32::max);
                (peak.clamp(0.0, 1.0) * 1000.0).round() / 1000.0
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_800_points() {
        let mut w = Waveform::new(2);
        let mut buf = Vec::new();
        for v in [0.1f32, 0.5, 0.3] {
            buf.extend(std::iter::repeat_n(v, 1024 * 2));
        }
        w.push(&buf);
        let out = w.finish();
        assert_eq!(out.len(), 800);
        for (i, &p) in out.iter().enumerate() {
            let expected = [0.1, 0.5, 0.3][i * 3 / 800];
            assert_eq!(p, expected, "point {i}");
        }
    }

    #[test]
    fn peak_uses_both_channels() {
        let mut w = Waveform::new(2);
        let buf: Vec<f32> = (0..1024).flat_map(|_| [0.2f32, -0.9]).collect();
        w.push(&buf);
        assert!(w.finish().iter().all(|&p| p == 0.9));
    }

    #[test]
    fn clamps_and_rounds() {
        let mut w = Waveform::new(1);
        w.push(&[1.7]);
        assert!(w.finish().iter().all(|&p| p == 1.0));
        let mut w = Waveform::new(1);
        w.push(&[0.12345]);
        assert!(w.finish().iter().all(|&p| p == 0.123));
    }
}
