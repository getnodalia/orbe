// SPDX-License-Identifier: MIT
//! Orbe's audio layer: sound data, a WAV decoder and a deterministic software mixer.
//!
//! This crate never touches a sound device. [`Mixer::mix`] fills a buffer of interleaved stereo
//! `f32` frames, and a platform backend hands that buffer to the operating system. Because mixing
//! is a pure function of the mixer's state, it is testable and reproducible.
//!
//! Sound files are hostile input: [`Sound::from_wav`] bounds every size it reads and returns an
//! error instead of panicking.

use std::fmt;

/// Most frames a decoded sound may hold (about 54 minutes at 48 kHz).
pub const MAX_FRAMES: usize = 48_000 * 60 * 54;
/// Most voices a mixer may play at once.
pub const MAX_VOICES: usize = 256;

/// Why a sound could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioError {
    /// The data is not a RIFF/WAVE file or is cut short.
    Malformed(&'static str),
    /// The encoding is valid but not supported.
    Unsupported(&'static str),
    /// The sound is longer than [`MAX_FRAMES`].
    TooLong,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::Malformed(m) => write!(f, "malformed WAV: {m}"),
            AudioError::Unsupported(m) => write!(f, "unsupported WAV: {m}"),
            AudioError::TooLong => write!(f, "sound is too long"),
        }
    }
}

impl std::error::Error for AudioError {}

/// A decoded sound: mono or stereo samples in `-1.0..=1.0`.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    sample_rate: u32,
    channels: u8,
    samples: Vec<f32>,
}

impl Sound {
    /// Builds a sound from interleaved samples. `channels` must be 1 or 2, `sample_rate` above
    /// zero, and the sample count a whole number of frames within [`MAX_FRAMES`].
    pub fn new(sample_rate: u32, channels: u8, samples: Vec<f32>) -> Option<Sound> {
        if sample_rate == 0 || !(1..=2).contains(&channels) {
            return None;
        }
        let ch = usize::from(channels);
        if samples.len() % ch != 0 || samples.len() / ch > MAX_FRAMES {
            return None;
        }
        Some(Sound {
            sample_rate,
            channels,
            samples,
        })
    }

    /// Samples per second per channel.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 1 for mono, 2 for stereo.
    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// Number of frames (one sample per channel each).
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// Length in seconds.
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate)
    }

    fn frame(&self, index: usize) -> (f32, f32) {
        let ch = usize::from(self.channels);
        let base = index * ch;
        let l = self.samples.get(base).copied().unwrap_or(0.0);
        let r = if ch == 2 {
            self.samples.get(base + 1).copied().unwrap_or(0.0)
        } else {
            l
        };
        (l, r)
    }

    /// Decodes a RIFF/WAVE file: PCM 8, 16, 24 or 32 bit and IEEE float 32 bit, mono or stereo.
    pub fn from_wav(data: &[u8]) -> Result<Sound, AudioError> {
        if data.get(0..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WAVE") {
            return Err(AudioError::Malformed("missing RIFF/WAVE header"));
        }
        let mut pos = 12usize;
        let mut fmt: Option<(u16, u16, u32, u16)> = None;
        let mut pcm: Option<&[u8]> = None;
        while let Some(header) = data.get(pos..pos.saturating_add(8)) {
            let id = header.get(0..4).unwrap_or(&[]);
            let size = u32_at(header, 4) as usize;
            let body_start = pos + 8;
            let body_end = body_start.saturating_add(size).min(data.len());
            let body = data.get(body_start..body_end).unwrap_or(&[]);
            if id == b"fmt " {
                if body.len() < 16 {
                    return Err(AudioError::Malformed("fmt chunk too short"));
                }
                fmt = Some((
                    u16_at(body, 0),
                    u16_at(body, 2),
                    u32_at(body, 4),
                    u16_at(body, 14),
                ));
            } else if id == b"data" {
                pcm = Some(body);
            }
            // Chunks are padded to an even size.
            pos = match body_start
                .checked_add(size)
                .and_then(|p| p.checked_add(size & 1))
            {
                Some(p) => p,
                None => break,
            };
        }
        let (format, channels, rate, bits) =
            fmt.ok_or(AudioError::Malformed("missing fmt chunk"))?;
        let pcm = pcm.ok_or(AudioError::Malformed("missing data chunk"))?;
        if !(1..=2).contains(&channels) {
            return Err(AudioError::Unsupported("only mono and stereo"));
        }
        if rate == 0 {
            return Err(AudioError::Malformed("zero sample rate"));
        }
        let bytes = usize::from(bits / 8);
        let is_float = format == 3;
        match (format, bits) {
            (1, 8) | (1, 16) | (1, 24) | (1, 32) | (3, 32) => {}
            _ => return Err(AudioError::Unsupported("encoding")),
        }
        let count = pcm.len() / bytes;
        let count = count - count % usize::from(channels);
        if count / usize::from(channels) > MAX_FRAMES {
            return Err(AudioError::TooLong);
        }
        let mut samples = Vec::with_capacity(count);
        for chunk in pcm.chunks_exact(bytes).take(count) {
            samples.push(decode_sample(chunk, bits, is_float));
        }
        Sound::new(rate, channels as u8, samples).ok_or(AudioError::Malformed("bad layout"))
    }
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    let lo = b.get(i).copied().unwrap_or(0);
    let hi = b.get(i + 1).copied().unwrap_or(0);
    u16::from_le_bytes([lo, hi])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    let g = |k: usize| b.get(i + k).copied().unwrap_or(0);
    u32::from_le_bytes([g(0), g(1), g(2), g(3)])
}

fn decode_sample(c: &[u8], bits: u16, is_float: bool) -> f32 {
    let g = |k: usize| c.get(k).copied().unwrap_or(0);
    let v = match bits {
        8 => (f32::from(g(0)) - 128.0) / 128.0,
        16 => f32::from(i16::from_le_bytes([g(0), g(1)])) / 32768.0,
        24 => {
            let raw = i32::from_le_bytes([0, g(0), g(1), g(2)]) >> 8;
            raw as f32 / 8_388_608.0
        }
        _ if is_float => f32::from_le_bytes([g(0), g(1), g(2), g(3)]),
        _ => i32::from_le_bytes([g(0), g(1), g(2), g(3)]) as f32 / 2_147_483_648.0,
    };
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Identifies a playing voice. Ids are never reused within one [`Mixer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoiceId(u64);

struct Voice {
    id: VoiceId,
    sound: std::sync::Arc<Sound>,
    /// Position in source frames.
    position: f64,
    volume: f32,
    pan: f32,
    pitch: f32,
    looping: bool,
}

/// Mixes voices into interleaved stereo `f32` frames at a fixed output rate.
pub struct Mixer {
    sample_rate: u32,
    master: f32,
    next_id: u64,
    voices: Vec<Voice>,
}

impl Mixer {
    /// A mixer producing audio at `sample_rate`, or `None` when that is zero.
    pub fn new(sample_rate: u32) -> Option<Mixer> {
        if sample_rate == 0 {
            return None;
        }
        Some(Mixer {
            sample_rate,
            master: 1.0,
            next_id: 0,
            voices: Vec::new(),
        })
    }

    /// Master volume, clamped to `0.0..=4.0`; non-finite values are ignored.
    pub fn set_master(&mut self, volume: f32) {
        if volume.is_finite() {
            self.master = volume.clamp(0.0, 4.0);
        }
    }

    /// Number of voices currently playing.
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Starts a sound. `volume` is clamped to `0.0..=4.0`, `pan` to `-1.0..=1.0` (left to right)
    /// and `pitch` to `0.01..=16.0`; non-finite values fall back to 1, 0 and 1. Returns `None`
    /// when [`MAX_VOICES`] are already playing.
    pub fn play(
        &mut self,
        sound: std::sync::Arc<Sound>,
        volume: f32,
        pan: f32,
        pitch: f32,
        looping: bool,
    ) -> Option<VoiceId> {
        if self.voices.len() >= MAX_VOICES {
            return None;
        }
        let id = VoiceId(self.next_id);
        self.next_id += 1;
        self.voices.push(Voice {
            id,
            sound,
            position: 0.0,
            volume: finite_or(volume, 1.0).clamp(0.0, 4.0),
            pan: finite_or(pan, 0.0).clamp(-1.0, 1.0),
            pitch: finite_or(pitch, 1.0).clamp(0.01, 16.0),
            looping,
        });
        Some(id)
    }

    /// Stops one voice. Returns whether it was playing.
    pub fn stop(&mut self, id: VoiceId) -> bool {
        let before = self.voices.len();
        self.voices.retain(|v| v.id != id);
        self.voices.len() != before
    }

    /// Whether a voice is still playing.
    pub fn is_playing(&self, id: VoiceId) -> bool {
        self.voices.iter().any(|v| v.id == id)
    }

    /// Fills `out` with interleaved stereo frames (any odd trailing sample is zeroed), advances
    /// every voice and drops the ones that finished.
    pub fn mix(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let master = self.master;
        let rate = f64::from(self.sample_rate);
        for voice in &mut self.voices {
            let frames = voice.sound.frames();
            if frames == 0 {
                voice.position = f64::INFINITY;
                continue;
            }
            let step = f64::from(voice.pitch) * f64::from(voice.sound.sample_rate()) / rate;
            // Equal-power pan.
            let angle = (f64::from(voice.pan) + 1.0) * std::f64::consts::FRAC_PI_4;
            let gl = (angle.cos() as f32) * voice.volume * master;
            let gr = (angle.sin() as f32) * voice.volume * master;
            for frame in out.chunks_exact_mut(2) {
                if voice.position >= frames as f64 {
                    if voice.looping {
                        voice.position %= frames as f64;
                    } else {
                        break;
                    }
                }
                let i = voice.position as usize;
                let t = (voice.position - i as f64) as f32;
                let (l0, r0) = voice.sound.frame(i);
                let next = if i + 1 < frames {
                    i + 1
                } else if voice.looping {
                    0
                } else {
                    i
                };
                let (l1, r1) = voice.sound.frame(next);
                let l = l0 + (l1 - l0) * t;
                let r = r0 + (r1 - r0) * t;
                if let [fl, fr] = frame {
                    *fl += l * gl;
                    *fr += r * gr;
                }
                voice.position += step;
            }
        }
        self.voices
            .retain(|v| v.looping && v.sound.frames() > 0 || v.position < v.sound.frames() as f64);
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }
}

fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn wav16(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        v.extend_from_slice(&(channels * 2).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }

    #[test]
    fn decodes_pcm16() {
        let s = Sound::from_wav(&wav16(8000, 1, &[0, 16384, -32768])).unwrap();
        assert_eq!(s.sample_rate(), 8000);
        assert_eq!(s.frames(), 3);
        assert_eq!(s.frame(1), (0.5, 0.5));
        assert_eq!(s.frame(2), (-1.0, -1.0));
    }

    #[test]
    fn rejects_garbage_and_truncation_without_panicking() {
        assert!(Sound::from_wav(b"").is_err());
        assert!(Sound::from_wav(b"RIFFxxxxWAVE").is_err());
        let good = wav16(8000, 2, &[1, 2, 3, 4]);
        for n in 0..good.len() {
            let _ = Sound::from_wav(&good[..n]);
        }
        // A data chunk that claims far more than the file holds is clamped, not trusted.
        let mut lie = good.clone();
        lie[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Sound::from_wav(&lie).is_ok());
    }

    #[test]
    fn rejects_unsupported_encodings() {
        let mut w = wav16(8000, 1, &[0, 0]);
        w[20] = 2; // ADPCM
        assert_eq!(
            Sound::from_wav(&w),
            Err(AudioError::Unsupported("encoding"))
        );
    }

    #[test]
    fn mixes_and_finishes() {
        let sound = Arc::new(Sound::new(10, 1, vec![0.5; 10]).unwrap());
        let mut m = Mixer::new(10).unwrap();
        let id = m.play(sound, 1.0, 0.0, 1.0, false).unwrap();
        let mut out = vec![0.0; 8];
        m.mix(&mut out);
        assert!(out.iter().all(|s| *s > 0.0));
        assert!(m.is_playing(id));
        m.mix(&mut out);
        m.mix(&mut out);
        assert!(!m.is_playing(id));
        assert_eq!(m.voice_count(), 0);
    }

    #[test]
    fn pan_moves_sound_between_channels() {
        let sound = Arc::new(Sound::new(10, 1, vec![1.0; 10]).unwrap());
        let mut m = Mixer::new(10).unwrap();
        m.play(sound, 1.0, -1.0, 1.0, true);
        let mut out = vec![0.0; 4];
        m.mix(&mut out);
        assert!(out[0] > 0.99 && out[1].abs() < 1e-6);
    }

    #[test]
    fn looping_wraps_and_hostile_values_are_sanitised() {
        let sound = Arc::new(Sound::new(4, 1, vec![0.25; 4]).unwrap());
        let mut m = Mixer::new(4).unwrap();
        m.set_master(f32::NAN);
        let id = m.play(sound, f32::NAN, f32::INFINITY, -5.0, true).unwrap();
        let mut out = vec![0.0; 64];
        m.mix(&mut out);
        assert!(m.is_playing(id));
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(m.stop(id));
        assert!(!m.stop(id));
    }

    #[test]
    fn voice_cap_is_enforced() {
        let sound = Arc::new(Sound::new(10, 1, vec![0.0; 10]).unwrap());
        let mut m = Mixer::new(10).unwrap();
        for _ in 0..MAX_VOICES {
            assert!(m.play(sound.clone(), 1.0, 0.0, 1.0, true).is_some());
        }
        assert!(m.play(sound, 1.0, 0.0, 1.0, true).is_none());
    }
}
