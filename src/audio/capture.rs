//! Audio for the deterministic video capture (`crate::capture`, the `capture` feature): the
//! game's one kira manager on a [`CaptureBackend`] instead of the sound card. Each recorded
//! frame renders exactly [`CAPTURE_RATE`]` / fps` samples of music + sfx (mixed by kira, as on
//! the device) into a WAV file ([`AudioCapture`]), so the audio follows game time, not the wall
//! clock, and its length is exactly frames / fps.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use kira::AudioManager;
use kira::backend::{Backend, Renderer};

/// The capture's sample rate (a 48 kHz "device").
pub const CAPTURE_RATE: u32 = 48_000;

/// Samples (stereo frames) per video frame at `fps`, or `None` if `fps` doesn't divide the
/// rate (then the audio couldn't stay sample-exact with the frames).
pub fn samples_per_frame(rate: u32, fps: u32) -> Option<usize> {
    (fps > 0 && rate.is_multiple_of(fps)).then(|| (rate / fps) as usize)
}

/// A kira backend with no device: whoever owns the manager pulls the audio
/// ([`render_block`]).
pub struct CaptureBackend {
    renderer: Option<Renderer>,
}

impl Backend for CaptureBackend {
    type Settings = ();
    type Error = ();

    fn setup(_settings: (), _internal_buffer_size: usize) -> Result<(Self, u32), ()> {
        Ok((CaptureBackend { renderer: None }, CAPTURE_RATE))
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), ()> {
        self.renderer = Some(renderer);
        Ok(())
    }
}

/// Render the next `out.len() / 2` stereo frames (interleaved L R) of the manager's mix, as one
/// device callback would.
pub fn render_block(manager: &mut AudioManager<CaptureBackend>, out: &mut [f32]) {
    let Some(r) = manager.backend_mut().renderer.as_mut() else {
        out.fill(0.0);
        return;
    };
    r.on_start_processing();
    r.process(out, 2);
}

/// The recording: where the WAV goes, how much to render per frame, and whether this frame is
/// recorded (`crate::capture` turns it on after the warm-up frame and off at the end).
#[derive(Resource)]
pub struct AudioCapture {
    /// Render a block this frame.
    pub recording: bool,
    block: usize,
    buf: Vec<f32>,
    wav: Option<WavWriter>,
    /// Blocks written.
    pub blocks: u64,
}

impl AudioCapture {
    /// Record into `path`, `CAPTURE_RATE / fps` samples per frame.
    pub fn new(path: &Path, fps: u32) -> io::Result<Self> {
        let block = samples_per_frame(CAPTURE_RATE, fps)
            .ok_or_else(|| io::Error::other(format!("fps {fps} must divide {CAPTURE_RATE}")))?;
        Ok(AudioCapture { recording: false, block, buf: vec![0.0; block * 2], wav: Some(WavWriter::create(path, CAPTURE_RATE)?), blocks: 0 })
    }

    /// Samples per frame.
    pub fn block(&self) -> usize {
        self.block
    }

    /// One frame's audio from the manager, into the file (if recording).
    pub fn pump(&mut self, manager: &mut AudioManager<CaptureBackend>) {
        if !self.recording {
            return;
        }
        render_block(manager, &mut self.buf);
        if let Some(w) = self.wav.as_mut()
            && let Err(e) = w.write(&self.buf)
        {
            error!("capture audio: {e}");
        }
        self.blocks += 1;
    }

    /// Stop and finish the file (idempotent). Returns its path.
    pub fn finish(&mut self) -> io::Result<Option<PathBuf>> {
        self.recording = false;
        match self.wav.take() {
            Some(w) => w.finish().map(Some),
            None => Ok(None),
        }
    }
}

/// A minimal streaming WAV writer: 32-bit float stereo.
pub struct WavWriter {
    out: BufWriter<File>,
    path: PathBuf,
    samples: u64,
}

impl WavWriter {
    pub fn create(path: &Path, rate: u32) -> io::Result<Self> {
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(&header(rate, 0))?;
        Ok(WavWriter { out, path: path.to_owned(), samples: 0 })
    }

    /// Interleaved stereo samples.
    pub fn write(&mut self, interleaved: &[f32]) -> io::Result<()> {
        for s in interleaved {
            self.out.write_all(&s.to_le_bytes())?;
        }
        self.samples += interleaved.len() as u64;
        Ok(())
    }

    /// Patch the sizes into the header.
    pub fn finish(mut self) -> io::Result<PathBuf> {
        self.out.flush()?;
        let rate = CAPTURE_RATE;
        let mut f = self.out.into_inner().map_err(|e| e.into_error())?;
        f.seek(SeekFrom::Start(0))?;
        f.write_all(&header(rate, self.samples * 4))?;
        f.sync_all()?;
        Ok(self.path)
    }
}

/// A 44-byte WAVE header for `data_bytes` of 32-bit float stereo at `rate`.
fn header(rate: u32, data_bytes: u64) -> [u8; 44] {
    let data = data_bytes.min(u32::MAX as u64 - 36) as u32;
    let mut h = [0u8; 44];
    let mut put = |at: usize, b: &[u8]| h[at..at + b.len()].copy_from_slice(b);
    put(0, b"RIFF");
    put(4, &(36 + data).to_le_bytes());
    put(8, b"WAVEfmt ");
    put(16, &16u32.to_le_bytes());
    put(20, &3u16.to_le_bytes()); // IEEE float
    put(22, &2u16.to_le_bytes());
    put(24, &rate.to_le_bytes());
    put(28, &(rate * 8).to_le_bytes());
    put(32, &8u16.to_le_bytes());
    put(34, &32u16.to_le_bytes());
    put(36, b"data");
    put(40, &data.to_le_bytes());
    h
}

/// The stereo frames of a WAV written by [`WavWriter`] (for tests and checks).
pub fn read_wav(path: &Path) -> io::Result<Vec<[f32; 2]>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[36..40] != b"data" {
        return Err(io::Error::other("not a capture WAV"));
    }
    let n = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let data = &bytes[44..44 + n.min(bytes.len() - 44)];
    Ok(data
        .chunks_exact(8)
        .map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap())])
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_divide_the_rate_exactly() {
        assert_eq!(samples_per_frame(48_000, 30), Some(1600));
        assert_eq!(samples_per_frame(48_000, 60), Some(800));
        assert_eq!(samples_per_frame(48_000, 24), Some(2000));
        assert_eq!(samples_per_frame(48_000, 7), None);
        assert_eq!(samples_per_frame(48_000, 0), None);
    }

    #[test]
    fn wav_round_trips() {
        let dir = std::env::temp_dir().join(format!("nathan-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.wav");
        let mut w = WavWriter::create(&p, CAPTURE_RATE).unwrap();
        w.write(&[0.5, -0.25, 1.0, 0.0]).unwrap();
        w.finish().unwrap();
        assert_eq!(read_wav(&p).unwrap(), vec![[0.5, -0.25], [1.0, 0.0]]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
