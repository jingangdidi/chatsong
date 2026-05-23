// Copyright 2026 Xiaomi Corp. (authors: Han Zhu)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! WAV I/O and audio processing utilities.
//!
//! Provides functions for loading, saving, resampling, silence removal,
//! and fade/padding of audio tensors. Audio is represented as candle
//! [`Tensor`] with shape `(1, num_samples)` and dtype `F32`.

use anyhow::{bail, Context, Result};
use candle_core::{DType, Device, Tensor};
use hound::{SampleFormat, WavReader, /*WavSpec, WavWriter*/};
/*
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
*/
use rubato::{
    Resampler,
    SincFixedIn,
    SincInterpolationParameters,
    SincInterpolationType,
    WindowFunction,
};
use std::path::Path;

/// Load a WAV file, convert to f32 mono, and resample to `target_sr`.
///
/// Returns a tensor of shape `(1, T)` on [`Device::Cpu`].
pub fn load_wav(path: impl AsRef<Path>, target_sr: usize) -> Result<Tensor> {
    let path = path.as_ref();
    let reader = WavReader::open(path)
        .with_context(|| format!("Failed to open WAV file: {}", path.display()))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;
    let source_sr = spec.sample_rate as usize;

    // Decode all samples to f32.
    let samples_f32: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => reader
            .into_samples::<i16>()
            .map(|s| s.map(|v| v as f32 / 32768.0))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        (SampleFormat::Int, 24) => reader
            .into_samples::<i32>()
            .map(|s| s.map(|v| v as f32 / 8_388_608.0))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        (SampleFormat::Int, 32) => reader
            .into_samples::<i32>()
            .map(|s| s.map(|v| v as f32 / 2_147_483_648.0))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        (SampleFormat::Float, _) => reader
            .into_samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        (fmt, bits) => bail!(
            "Unsupported WAV format: {:?} with {} bits per sample",
            fmt,
            bits
        ),
    };

    // Mix down to mono by averaging channels.
    let mono = if channels == 1 {
        samples_f32
    } else {
        samples_f32
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    // Resample if source rate differs from target rate.
    let mono = if source_sr != target_sr {
        resample(&mono, source_sr, target_sr)?
    } else {
        mono
    };

    // Build tensor of shape (1, T).
    let len = mono.len();
    Tensor::from_vec(mono, (1, len), &Device::Cpu).map_err(Into::into)
}

/*
/// Save an f32 tensor of shape `(1, T)` as a 16-bit PCM WAV file.
pub fn save_wav(path: impl AsRef<Path>, tensor: &Tensor, sample_rate: usize) -> Result<()> {
    let path = path.as_ref();
    let tensor = tensor.to_dtype(DType::F32)?.to_device(&Device::Cpu)?;

    // Flatten to 1-D.
    let data = tensor.flatten_all()?.to_vec1::<f32>()?;

    let spec = WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)
        .with_context(|| format!("Failed to create WAV file: {}", path.display()))?;

    for &s in &data {
        let clamped = s.clamp(-1.0, 1.0);
        let i16_val = (clamped * 32767.0) as i16;
        writer.write_sample(i16_val)?;
    }

    writer.finalize()?;
    Ok(())
}
*/

/// Resample a mono f32 buffer from `from_sr` to `to_sr` using a sinc
/// interpolation resampler ([`rubato::Async`]).
pub fn resample(samples: &[f32], from_sr: usize, to_sr: usize) -> Result<Vec<f32>> {
    if from_sr == to_sr {
        return Ok(samples.to_vec());
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        to_sr as f64 / from_sr as f64,
        2.0,
        params,
        samples.len(),
        1,
    ).unwrap();

    let resampled = resampler.process(&[samples], None)?;
    Ok(resampled.into_iter().next().unwrap_or_default())
}

/// Remove silence from an audio tensor using RMS-based detection.
///
/// * `audio` -- tensor of shape `(1, T)`, dtype `F32`
/// * `sr` -- sample rate
/// * `mid_sil_ms` -- middle silence segments longer than this are trimmed
/// * `lead_sil_ms` -- leading silence to keep
/// * `trail_sil_ms` -- trailing silence to keep
///
/// The silence threshold is -50 dB relative to full scale.
pub fn remove_silence(
    audio: &Tensor,
    sr: usize,
    mid_sil_ms: usize,
    lead_sil_ms: usize,
    trail_sil_ms: usize,
) -> Result<Tensor> {
    let samples = audio
        .to_dtype(DType::F32)?
        .to_device(&Device::Cpu)?
        .flatten_all()?
        .to_vec1::<f32>()?;

    if samples.is_empty() {
        return Ok(audio.clone());
    }

    // RMS threshold: -50 dB relative to full-scale (1.0).
    // -50 dB = 10^(-50/20) ~ 0.00316
    let rms_threshold: f32 = 10.0_f32.powf(-50.0 / 20.0);

    // Window size for RMS computation: 10 ms.
    let window_size = (sr as f32 * 0.01) as usize;
    let window_size = window_size.max(1);

    // Classify each window as silent or not.
    let num_windows = samples.len().div_ceil(window_size);
    let mut is_silent = vec![false; num_windows];

    for (i, chunk) in samples.chunks(window_size).enumerate() {
        let rms = (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();
        is_silent[i] = rms < rms_threshold;
    }

    // Find non-silent regions (runs of non-silent windows).
    let mut regions: Vec<(usize, usize)> = Vec::new(); // (start_sample, end_sample)
    let mut in_region = false;
    let mut region_start = 0usize;

    for (i, &silent) in is_silent.iter().enumerate() {
        if !silent && !in_region {
            region_start = i * window_size;
            in_region = true;
        } else if silent && in_region {
            let region_end = i * window_size;
            regions.push((region_start, region_end));
            in_region = false;
        }
    }
    if in_region {
        regions.push((region_start, samples.len()));
    }

    if regions.is_empty() {
        // Entire audio is silent; return as-is.
        return Ok(audio.clone());
    }

    // Remove middle silence gaps longer than mid_sil_ms.
    let mid_sil_samples = sr * mid_sil_ms / 1000;
    let keep_sil_samples = mid_sil_samples; // keep up to mid_sil_ms of silence between regions

    let mut output: Vec<f32> = Vec::with_capacity(samples.len());

    for (idx, &(start, end)) in regions.iter().enumerate() {
        if idx > 0 {
            let prev_end = regions[idx - 1].1;
            let gap = start.saturating_sub(prev_end);
            // Insert at most keep_sil_samples of the original silence.
            let kept = gap.min(keep_sil_samples);
            output.extend_from_slice(&samples[prev_end..prev_end + kept]);
        }
        let start = start.min(samples.len());
        let end = end.min(samples.len());
        output.extend_from_slice(&samples[start..end]);
    }

    // Handle leading silence: keep at most lead_sil_ms of silence before
    // the first non-silent region.
    let lead_sil_samples = sr * lead_sil_ms / 1000;
    let first_nonsilent = regions[0].0;
    let leading_keep = first_nonsilent.min(lead_sil_samples);
    let mut final_output: Vec<f32> = Vec::with_capacity(output.len() + lead_sil_samples);
    if leading_keep > 0 {
        let lead_start = first_nonsilent - leading_keep;
        final_output.extend_from_slice(&samples[lead_start..first_nonsilent]);
    }
    final_output.extend_from_slice(&output);

    // Handle trailing silence: keep at most trail_sil_ms after the last
    // non-silent region.
    let trail_sil_samples = sr * trail_sil_ms / 1000;
    let last_end = regions.last().map(|r| r.1).unwrap_or(0);
    let trailing_available = samples.len().saturating_sub(last_end);
    let trailing_keep = trailing_available.min(trail_sil_samples);
    if trailing_keep > 0 {
        final_output.extend_from_slice(&samples[last_end..last_end + trailing_keep]);
    }

    let len = final_output.len();
    Tensor::from_vec(final_output, (1, len), &Device::Cpu).map_err(Into::into)
}

/// Apply linear fade-in / fade-out and zero-pad both edges.
///
/// * `audio` -- tensor of shape `(1, T)`, dtype `F32`
/// * `pad_dur` -- duration of zero-padding on each side (seconds)
/// * `fade_dur` -- duration of the linear fade curve (seconds)
/// * `sr` -- sample rate
pub fn fade_and_pad(audio: &Tensor, pad_dur: f64, fade_dur: f64, sr: usize) -> Result<Tensor> {
    let num_samples = audio.dim(1)?;
    if num_samples == 0 {
        return Ok(audio.clone());
    }

    let fade_samples = (fade_dur * sr as f64) as usize;
    let pad_samples = (pad_dur * sr as f64) as usize;

    let mut data = audio
        .to_dtype(DType::F32)?
        .to_device(&Device::Cpu)?
        .flatten_all()?
        .to_vec1::<f32>()?;

    // Apply fade-in and fade-out.
    if fade_samples > 0 {
        let k = fade_samples.min(data.len() / 2);
        if k > 0 {
            // Fade in.
            for (i, sample) in data.iter_mut().take(k).enumerate() {
                let factor = i as f32 / k as f32;
                *sample *= factor;
            }
            // Fade out.
            let start = data.len() - k;
            for (i, sample) in data[start..].iter_mut().enumerate() {
                let factor = 1.0 - (i as f32 / k as f32);
                *sample *= factor;
            }
        }
    }

    // Zero-pad both sides.
    if pad_samples > 0 {
        let mut padded = vec![0.0f32; pad_samples + data.len() + pad_samples];
        padded[pad_samples..pad_samples + data.len()].copy_from_slice(&data);
        data = padded;
    }

    let len = data.len();
    Tensor::from_vec(data, (1, len), &Device::Cpu).map_err(Into::into)
}

/// Concatenate audio chunks with silence gaps and cross-fade at boundaries.
///
/// Each boundary: fade-out tail → silence buffer → fade-in head.
///
/// * `chunks` -- list of audio tensors, each `(1, T)` on CPU, F32
/// * `sample_rate` -- audio sample rate
/// * `silence_duration` -- total silence gap in seconds (default 0.3)
pub fn cross_fade_chunks(
    chunks: &[Tensor],
    sample_rate: usize,
    silence_duration: f64,
) -> Result<Tensor> {
    if chunks.len() == 1 {
        return Ok(chunks[0].clone());
    }

    let total_n = (silence_duration * sample_rate as f64) as usize;
    let fade_n = total_n / 3;
    let silence_n = fade_n;

    let mut merged: Vec<f32> = chunks[0].to_dtype(DType::F32)?.flatten_all()?.to_vec1()?;

    for chunk in &chunks[1..] {
        // Fade out tail of merged
        let fout_n = fade_n.min(merged.len());
        if fout_n > 0 {
            let start = merged.len() - fout_n;
            for (i, sample) in merged[start..].iter_mut().enumerate() {
                let w = 1.0 - (i as f32 / fout_n as f32);
                *sample *= w;
            }
        }

        // Silence gap
        merged.extend(std::iter::repeat_n(0.0f32, silence_n));

        // Fade in head of next chunk
        let mut chunk_data: Vec<f32> = chunk.to_dtype(DType::F32)?.flatten_all()?.to_vec1()?;
        let fin_n = fade_n.min(chunk_data.len());
        if fin_n > 0 {
            for (i, sample) in chunk_data[..fin_n].iter_mut().enumerate() {
                let w = i as f32 / fin_n as f32;
                *sample *= w;
            }
        }

        merged.extend_from_slice(&chunk_data);
    }

    let len = merged.len();
    Tensor::from_vec(merged, (1, len), &Device::Cpu).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: create a temporary WAV file with a sine wave and return its path.
    fn make_test_wav(sr: u32, duration_ms: u32, freq: f32) -> tempfile::NamedTempFile {
        let spec = WavSpec {
            channels: 1,
            sample_rate: sr,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = WavWriter::new(&mut tmp, spec).unwrap();
            let num_samples = (sr as f64 * duration_ms as f64 / 1000.0) as usize;
            for i in 0..num_samples {
                let t = i as f32 / sr as f32;
                let sample = (2.0 * std::f32::consts::PI * freq * t).sin();
                writer.write_sample((sample * 32767.0) as i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        // Flush to ensure data is written.
        tmp.as_file().flush().unwrap();
        tmp
    }

    #[test]
    fn test_load_wav_native_sr() {
        let tmp = make_test_wav(24000, 500, 440.0);
        let tensor = load_wav(tmp.path(), 24000).unwrap();
        assert_eq!(tensor.dims().len(), 2);
        assert_eq!(tensor.dim(0).unwrap(), 1);
        // 500 ms at 24000 Hz = 12000 samples
        assert_eq!(tensor.dim(1).unwrap(), 12000);
    }

    #[test]
    fn test_load_wav_resample() {
        let tmp = make_test_wav(16000, 1000, 440.0);
        let tensor = load_wav(tmp.path(), 24000).unwrap();
        assert_eq!(tensor.dim(0).unwrap(), 1);
        // 1 second at 24000 Hz should be approximately 24000 samples.
        let len = tensor.dim(1).unwrap();
        assert!((23500..24500).contains(&len), "Unexpected length: {len}");
    }

    #[test]
    fn test_save_and_reload() {
        let tmp = make_test_wav(24000, 200, 440.0);
        let tensor = load_wav(tmp.path(), 24000).unwrap();

        let out = tempfile::NamedTempFile::new().unwrap();
        save_wav(out.path(), &tensor, 24000).unwrap();

        let reloaded = load_wav(out.path(), 24000).unwrap();
        assert_eq!(tensor.dim(1).unwrap(), reloaded.dim(1).unwrap());
    }

    #[test]
    fn test_resample_identity() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 100.0).sin()).collect();
        let result = resample(&samples, 16000, 16000).unwrap();
        assert_eq!(result.len(), samples.len());
    }

    #[test]
    fn test_fade_and_pad() {
        let data: Vec<f32> = vec![1.0; 1000];
        let tensor = Tensor::from_vec(data, (1, 1000), &Device::Cpu).unwrap();
        let result = fade_and_pad(&tensor, 0.01, 0.01, 1000).unwrap();
        // pad = 10 samples each side, total = 1000 + 20 = 1020
        assert_eq!(result.dim(1).unwrap(), 1020);
        // First sample should be zero (padding).
        let vals = result.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        assert_eq!(vals[0], 0.0);
        assert_eq!(vals[vals.len() - 1], 0.0);
    }

    #[test]
    fn test_remove_silence_preserves_nonsilent() {
        // Create a short non-silent signal.
        let samples: Vec<f32> = (0..4800).map(|i| (i as f32 / 100.0).sin() * 0.5).collect();
        let tensor = Tensor::from_vec(samples, (1, 4800), &Device::Cpu).unwrap();
        let result = remove_silence(&tensor, 24000, 200, 100, 200).unwrap();
        // Should preserve most of the audio.
        assert!(result.dim(1).unwrap() > 0);
    }
}
