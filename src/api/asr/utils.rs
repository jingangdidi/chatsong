#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use std::path::Path;

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use hound::WavReader;

use hound::{
    WavSpec,
    WavWriter,
    SampleFormat,
};

use rubato::{
    Resampler,
    SincFixedIn,
    SincInterpolationParameters,
    SincInterpolationType,
    WindowFunction,
};

use crate::error::MyError;

/// resample audio from source_sr to target_sr
pub fn resample_audio(samples: Vec<f32>, source_sr: f64, target_sr: f64) -> Result<Vec<f32>, MyError> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        target_sr / source_sr,
        2.0,
        params,
        samples.len(),
        1,
    ).unwrap();

    let resampled = resampler.process(&[samples], None).map_err(|e| MyError::RunResamplerError{error: e})?;
    Ok(resampled.into_iter().next().unwrap_or_default())
}

// read wav file
#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
pub fn read_wav_sample_resample(path: &Path, target_sr: u32) -> Result<Vec<f32>, MyError> {
    let mut reader = WavReader::open(path).map_err(|e| MyError::WavError{error: e})?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let source_sr = spec.sample_rate;

    // Decode all samples to f32.
    let samples_f32: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect(),
        (SampleFormat::Int, 24) => reader.samples::<i32>().map(|s| s.unwrap() as f32 / 8_388_608.0).collect(),
        (SampleFormat::Int, 32) => reader.samples::<i32>().map(|s| s.unwrap() as f32 / 2_147_483_648.0).collect(),
        (SampleFormat::Float, _) => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        (fmt, bits) => return Err(MyError::OtherError{info: format!("Unsupported WAV format: {:?} with {} bits per sample", fmt, bits)}),
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
        resample_audio(mono, source_sr as f64, target_sr as f64)?
    } else {
        mono
    };

    Ok(mono)
}

// save wav file
pub fn write_wav_sample(samples: &[f32], sample_rate: u32, audio_id: usize, is_user: bool, audio_save_path: &str) -> Result<(), MyError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let file_path = format!("{}/{}_{}.wav", audio_save_path, if is_user { "me" } else { "model" }, audio_id);
    let mut writer = WavWriter::create(&file_path, spec).map_err(|e| MyError::WavError{error: e})?;
    let pcm_data_i16: Vec<i16> = samples.iter().map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).collect();
    for &sample in pcm_data_i16.iter() {
        writer.write_sample(sample).map_err(|e| MyError::WavError{error: e})?;
    }
    writer.finalize().map_err(|e| MyError::WavError{error: e})?;
    Ok(())
}
