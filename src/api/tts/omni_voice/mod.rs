use std::path::PathBuf;

use anyhow::{anyhow, Result};
use candle_core::{DType, Device};
use tokio::sync::mpsc::{
    Sender,
    Receiver,
};
use tokio::sync::oneshot;
use tracing::{event, Level};

mod config;
mod models;
mod utils;

use config::{HiggsAudioV2Config, OmniVoiceConfig};
use models::higgs_audio_v2::HiggsAudioV2Tokenizer;
use models::omnivoice::{GenerateRequest, GenerationConfig, OmniVoice};
use utils::audio::{fade_and_pad, load_wav, remove_silence, /*save_wav*/};
use utils::duration::RuleDurationEstimator;
use utils::text::{add_punctuation, combine_text, is_cjk};
use utils::voice_design::resolve_instruct;
use utils::audio::cross_fade_chunks;

/// Resolve language name or code to ISO 639-1 code using `isolang`.
/// Accepts: ISO 639-1 ("en"), ISO 639-3 ("eng"), or full English name ("English").
fn resolve_language(lang: Option<&str>) -> Option<String> {
    let lang = lang?;
    if lang.eq_ignore_ascii_case("none") {
        return None;
    }
    // Try ISO 639-1 code first (e.g. "en")
    if let Some(l) = isolang::Language::from_639_1(lang) {
        return l.to_639_1().map(|s| s.to_string());
    }
    // Try ISO 639-3 code (e.g. "eng")
    if let Some(l) = isolang::Language::from_639_3(lang) {
        return l.to_639_1().map(|s| s.to_string());
    }
    // Try full English name (e.g. "English", "Chinese")
    if let Some(l) = isolang::Language::from_name(lang) {
        if let Some(code) = l.to_639_1() {
            return Some(code.to_string());
        }
    }
    // Case-insensitive name lookup via FromStr (requires lowercase_names feature)
    if let Ok(l) = lang.parse::<isolang::Language>() {
        if let Some(code) = l.to_639_1() {
            return Some(code.to_string());
        }
    }
    event!(Level::WARN, "Unknown language '{}'. Use ISO 639-1 (en, zh, ja), ISO 639-3 (eng, zho, jpn), or English name (English, Chinese).", lang);
    Some(lang.to_string())
}

pub async fn run_omni_tts(
    path: &str,
    ref_audio: Option<PathBuf>,
    ref_text: Option<String>,
    mut rx_tts_string: Receiver<(String, Option<String>, Option<String>)>,
    tx_tts_audio: Sender<(Vec<f32>, usize, oneshot::Sender<()>)>,
) -> Result<(), anyhow::Error> {
    let device = {
        #[cfg(feature = "tts-cuda")]
        {
            match candle_core::Device::new_cuda(0) {
                Ok(device) => Ok(device),
                Err(e) => Err(anyhow!(format!("CUDA feature enabled but device creation failed: {}", e))),
            }
        }
        #[cfg(feature = "tts-metal")]
        match candle_core::Device::new_cuda(0) {
            Ok(device) => Ok(device),
            Err(e) => Err(anyhow!(format!("CUDA feature enabled but device creation failed: {}", e))),
        }
        #[cfg(feature = "tts")]
        Ok::<candle_core::Device, anyhow::Error>(candle_core::Device::Cpu)
    }?;

    let dtype = if device.is_cpu() {
        DType::F32
    } else {
        DType::F16
    };

    // 1. Resolve model path
    let model_dir = PathBuf::from(path);
    if !model_dir.is_dir() {
        return Err(anyhow!("Failed to resolve model directory".to_string()))
    }

    // 2. Load configs
    let config_path = model_dir.join("config.json");
    let config: OmniVoiceConfig = serde_json::from_reader(std::fs::File::open(&config_path)?)?;

    let audio_config_path = model_dir.join("audio_tokenizer/config.json");
    let audio_config: HiggsAudioV2Config = serde_json::from_reader(std::fs::File::open(&audio_config_path)?)?;

    // 3. Load text tokenizer
    let tokenizer_path = model_dir.join("tokenizer.json");
    let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;

    // 4. Load OmniVoice model
    let model_weights = model_dir.join("model.safetensors");
    // SAFETY: The safetensors file was either downloaded from HuggingFace Hub
    // (validated by hf-hub) or provided as a local path by the user. The file
    // must remain unmodified while memory-mapped. This is the standard candle
    // pattern for loading model weights.
    let vb = unsafe {
        candle_nn::VarBuilder::from_mmaped_safetensors(&[model_weights], dtype, &device)?
    };
    let model = OmniVoice::new(&config, vb)?;

    // 5. Load HiggsAudioV2 tokenizer
    // Audio tokenizer always runs on CPU in F32 — Metal/MPS don't support the
    // Snake1d activations and conv_transpose1d at F16 precision, matching the
    // Python behavior (which also forces audio_tokenizer to CPU for MPS).
    let audio_weights = model_dir.join("audio_tokenizer/model.safetensors");
    // SAFETY: Same as above — file is from HuggingFace or user-supplied local path.
    let audio_vb = unsafe {
        candle_nn::VarBuilder::from_mmaped_safetensors(&[audio_weights], DType::F32, &Device::Cpu)?
    };
    let audio_tokenizer = HiggsAudioV2Tokenizer::new(&audio_config, audio_vb)?;

    // 6. Process reference audio if provided
    let (ref_audio_tokens, ref_text, ref_rms) = if let Some(ref_audio_path) = ref_audio {
        let sampling_rate = audio_config.sample_rate();
        let wav = load_wav(ref_audio_path, sampling_rate)?;

        // Compute RMS for volume normalization
        let rms = (wav.sqr()?.mean_all()?.sqrt()?.to_scalar::<f32>()?) as f64;

        // Normalize quiet audio before encoding (matches Python create_voice_clone_prompt)
        let wav = if rms > 0.0 && rms < 0.1 {
            (&wav * (0.1 / rms))?
        } else {
            wav
        };

        // Trim long audio (>20s) — only when ref_text not provided (auto-transcribe case)
        // When ref_text IS provided, warn but don't trim (matches Python behavior)
        let max_ref_seconds = 20.0;
        let wav_dur = wav.dim(1)? as f64 / sampling_rate as f64;
        let wav = if wav_dur > max_ref_seconds && ref_text.is_none() {
            let max_samples = (max_ref_seconds * sampling_rate as f64) as usize;
            event!(Level::INFO, "Trimming reference audio from {wav_dur:.1}s to {max_ref_seconds}s");
            wav.narrow(1, 0, max_samples.min(wav.dim(1)?))?
        } else if wav_dur > max_ref_seconds {
            event!(Level::WARN, "Reference audio is {wav_dur:.1}s (>{max_ref_seconds}s). Long references may degrade quality.");
            wav
        } else {
            wav
        };

        // Silence removal
        let wav = remove_silence(&wav, sampling_rate, 200, 100, 200)?;

        // Clip to multiple of hop_length
        let hop_length = audio_config.hop_length();
        let len = wav.dim(1)?;
        let clip = len % hop_length;
        let wav = if clip > 0 {
            wav.narrow(1, 0, len - clip)?
        } else {
            wav
        };

        // Encode reference audio to tokens (audio tokenizer is on CPU)
        let tokens = audio_tokenizer.encode(&wav.unsqueeze(0)?)?;
        let tokens = tokens.squeeze(0)?; // (8, T)

        let ref_text_str = ref_text.clone().unwrap_or_default();
        let ref_text_str = if ref_text_str.is_empty() {
            return Err(anyhow!("ref audio need ref text".to_string()))
        } else {
            add_punctuation(&ref_text_str)
        };

        (Some(tokens), Some(ref_text_str), Some(rms))
    } else {
        (None, None, None)
    };

    // 7. Estimate target duration
    let duration_estimator = RuleDurationEstimator::new();
    let frame_rate = audio_config.frame_rate();

    // 等待接收内容
    event!(Level::INFO, "tts waiting new text...");
    while let Some((content, voice_design, lang)) = rx_tts_string.recv().await {
        //let num_target_tokens = if let Some(dur) = args.duration {
        let num_target_tokens = {
            let (est_ref_text, est_ref_tokens) =
                if let (Some(rt), Some(rat)) = (&ref_text, &ref_audio_tokens) {
                    (rt.as_str(), rat.dim(1)?)
                } else {
                    //("Nice to meet you.", 25)
                    ("该模型基于一种新颖的扩散语言模型架构", 80)
                };
            let est =
                duration_estimator.estimate_duration(&content, est_ref_text, est_ref_tokens as f64);
            /*
            let est = if args.speed > 0.0 && args.speed != 1.0 {
                est / args.speed
            } else {
                est
            };
            */
            est.max(1.0) as usize // truncation, matching Python's int()
        };
        event!(Level::INFO, "Target audio tokens: {num_target_tokens}");

        if num_target_tokens < 30 {
            event!(Level::WARN, "Very short text ({num_target_tokens} tokens). The model may produce low-quality audio for texts shorter than ~5 words. Consider adding more content.");
        }

        // 8. Validate instruct and resolve language
        let use_zh = content.chars().any(is_cjk);
        let instruct = match &voice_design {
            Some(s) => resolve_instruct(Some(s), use_zh)?,
            None => None,
        };
        let language = resolve_language(lang.as_deref());

        let gen_config = GenerationConfig {
            num_step: 16, //32, // num_step: Number of iterative decoding steps
            guidance_scale: 2.0, // guidance_scale: Classifier-free guidance scale
            t_shift: 0.1, // t_shift: Noise schedule time shift
            layer_penalty_factor: 5.0, // layer_penalty_factor: Layer penalty factor (encourage lower codebook layers first)
            position_temperature: 5.0, // position_temperature: Temperature for position selection (Gumbel noise)
            class_temperature: 0.0, // class_temperature: Temperature for token sampling (0 = greedy)
            denoise: true, // denoise: Whether to prepend the denoise token
            ..GenerationConfig::default()
        };

        let full_text = combine_text(&content, ref_text.as_deref());

        let chunk_tokens = model.generate(&GenerateRequest {
            tokenizer: &tokenizer,
            full_text: &full_text,
            num_target_tokens,
            ref_audio_tokens: ref_audio_tokens.as_ref(),
            ref_text: ref_text.as_deref(),
            lang: language.as_deref(),
            instruct: instruct.as_deref(),
            gen_config: &gen_config,
            frame_rate,
            speed: 1.0, // speed: Speaking speed factor
            duration_estimator: &duration_estimator,
            device: &device,
            dtype,
        })?;

        // 9. Decode token chunks to waveform (audio tokenizer runs on CPU)
        event!(Level::INFO, "Decoding {} audio chunk(s)...", chunk_tokens.len());
        let chunk_audios: Vec<candle_core::Tensor> = chunk_tokens
            .iter()
            .map(|t| {
                let cpu_tokens = t.to_device(&Device::Cpu)?;
                let decoded = audio_tokenizer.decode(&cpu_tokens.unsqueeze(0)?)?;
                decoded.squeeze(0)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let audio = if chunk_audios.len() == 1 {
            chunk_audios.into_iter().next().unwrap()
        } else {
            cross_fade_chunks(&chunk_audios, audio_config.sample_rate(), 0.3)?
        };

        // 10. Post-process
        let sampling_rate = audio_config.sample_rate();
        /*
        let audio = if args.postprocess_output {
            remove_silence(&audio, sampling_rate, 500, 100, 100)?
        } else {
            audio
        };
        */
        // Post-process output (silence removal, fade)
        let audio = remove_silence(&audio, sampling_rate, 500, 100, 100)?;

        // Volume normalization
        let audio = if let Some(rms) = ref_rms {
            if rms < 0.1 {
                (audio * (rms / 0.1))?
            } else {
                audio
            }
        } else {
            // Voice design mode: peak-normalize to 0.5
            let peak = audio.abs()?.max(1)?.max(0)?.to_scalar::<f32>()?;
            if peak > 1e-6 {
                ((audio / (peak as f64))? * 0.5)?
            } else {
                audio
            }
        };

        let audio = fade_and_pad(&audio, 0.1, 0.1, sampling_rate)?;

        // 11. Save WAV
        let audio_f32 = audio.to_dtype(DType::F32)?;
        /*
        save_wav("output-omni.wav", &audio_f32, sampling_rate)?;
        info!("Saved to {}", "output-omni.wav");
        */

        let (ack_tts_tx, ack_tts_rx) = oneshot::channel(); // 创建应答通道
        if let Err(_) = tx_tts_audio.send((
                audio_f32.to_dtype(DType::F32)?.to_device(&Device::Cpu)?.flatten_all()?.to_vec1::<f32>()?,
                sampling_rate,
                ack_tts_tx,
            )).await {
            event!(Level::ERROR, "tts receiver dropped");
        }
        ack_tts_rx.await.unwrap(); // 等待接收方确认
    }

    Ok(())
}
