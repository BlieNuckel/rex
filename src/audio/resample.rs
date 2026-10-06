use anyhow::Result;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler as _};

const CH: usize = 2;
const CHUNK: usize = 1024;

pub struct Resampler {
    inner: Fft<f32>,
    from: u32,
    ratio: f64,
    pending: Vec<f32>,
    out: Vec<f32>,
    to_skip: usize,
    fed: u64,
    emitted: u64,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Result<Self> {
        let inner = Fft::<f32>::new(from as usize, to as usize, CHUNK, CH, FixedSync::Input)?;
        Ok(Self {
            to_skip: inner.output_delay(),
            out: vec![0.0; inner.output_frames_max() * CH],
            inner,
            from,
            ratio: to as f64 / from as f64,
            pending: Vec::with_capacity(CHUNK * CH * 2),
            fed: 0,
            emitted: 0,
        })
    }

    pub fn input_rate(&self) -> u32 {
        self.from
    }

    /// Output frames still owed for input already pushed (buffered input plus filter delay)
    pub fn owed(&self) -> u64 {
        ((self.fed as f64 * self.ratio).round() as u64).saturating_sub(self.emitted)
    }

    pub fn push(&mut self, input: &[f32], dst: &mut Vec<f32>) -> Result<()> {
        self.pending.extend_from_slice(input);
        self.fed += (input.len() / CH) as u64;
        self.drain(dst, u64::MAX)
    }

    /// Emits everything still buffered, padding with silence, then resets for reuse
    pub fn flush(&mut self, dst: &mut Vec<f32>) -> Result<()> {
        let expected = (self.fed as f64 * self.ratio).round() as u64;
        while self.emitted < expected {
            let need = self.inner.input_frames_next() * CH;
            let pad = need - self.pending.len() % need;
            if pad != need || self.pending.is_empty() {
                self.pending.resize(self.pending.len() + pad, 0.0);
            }
            self.drain(dst, expected)?;
        }
        self.reset();
        Ok(())
    }

    pub fn reset(&mut self) {
        self.inner.reset();
        self.pending.clear();
        self.to_skip = self.inner.output_delay();
        self.fed = 0;
        self.emitted = 0;
    }

    fn drain(&mut self, dst: &mut Vec<f32>, limit: u64) -> Result<()> {
        let frames = self.pending.len() / CH;
        let mut offset = 0;
        loop {
            let need = self.inner.input_frames_next();
            if frames - offset < need || self.emitted >= limit {
                break;
            }
            let input = InterleavedSlice::new(&self.pending, CH, frames)?;
            let out_frames = self.out.len() / CH;
            let mut output = InterleavedSlice::new_mut(&mut self.out, CH, out_frames)?;
            let idx = Indexing {
                input_offset: offset,
                output_offset: 0,
                partial_len: None,
                active_channels_mask: None,
            };
            let (used, made) = self
                .inner
                .process_into_buffer(&input, &mut output, Some(&idx))?;
            offset += used;
            let skip = self.to_skip.min(made);
            self.to_skip -= skip;
            let keep = ((made - skip) as u64).min(limit - self.emitted) as usize;
            dst.extend_from_slice(&self.out[skip * CH..(skip + keep) * CH]);
            self.emitted += keep as u64;
        }
        self.pending.drain(..offset * CH);
        Ok(())
    }
}
