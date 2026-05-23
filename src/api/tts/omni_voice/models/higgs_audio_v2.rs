//! HiggsAudioV2 Tokenizer: encode audio to discrete tokens, decode tokens to audio.
//!
//! Composes DAC (acoustic) + HuBERT (semantic) + Semantic Codec + RVQ.

use candle_core::{DType, Module, Result, Tensor};
use candle_nn::VarBuilder;

use crate::tts::omni_voice::{
    config::HiggsAudioV2Config,
    models::dac::{DacDecoder, DacEncoder},
    models::hubert::HuBERTModel,
    models::rvq::ResidualVectorQuantization,
    models::semantic_codec::{SemanticDecoder, SemanticEncoder},
    utils::audio::resample,
};

#[derive(Debug, Clone)]
pub struct HiggsAudioV2Tokenizer {
    acoustic_encoder: DacEncoder,
    acoustic_decoder: DacDecoder,
    semantic_model: HuBERTModel,
    encoder_semantic: SemanticEncoder,
    /// Loaded from weights for completeness; not used in TTS inference decode path.
    #[allow(dead_code)]
    decoder_semantic: SemanticDecoder,
    fc: candle_nn::Linear,
    /// Loaded from weights for completeness; semantic decode projection.
    #[allow(dead_code)]
    fc1: candle_nn::Linear,
    fc2: candle_nn::Linear,
    quantizer: ResidualVectorQuantization,
    config: HiggsAudioV2Config,
}

impl HiggsAudioV2Tokenizer {
    pub fn new(cfg: &HiggsAudioV2Config, vb: VarBuilder) -> Result<Self> {
        let dac_cfg = cfg.dac_config();
        let hubert_cfg = cfg.hubert_config();

        let acoustic_encoder = DacEncoder::new(dac_cfg, vb.pp("acoustic_encoder"))?;
        let acoustic_decoder = DacDecoder::new(dac_cfg, vb.pp("acoustic_decoder"))?;
        let semantic_model = HuBERTModel::new(hubert_cfg, vb.pp("semantic_model"))?;
        let encoder_semantic = SemanticEncoder::new(cfg, vb.pp("encoder_semantic"))?;
        let decoder_semantic = SemanticDecoder::new(cfg, vb.pp("decoder_semantic"))?;

        let hidden = cfg.hidden_size();
        let sem_hidden = cfg.semantic_hidden_size();
        let ac_hidden = dac_cfg.hidden_size;

        let fc = candle_nn::linear(hidden, hidden, vb.pp("fc"))?;
        let fc1 = candle_nn::linear(hidden, sem_hidden, vb.pp("fc1"))?;
        let fc2 = candle_nn::linear(hidden, ac_hidden, vb.pp("fc2"))?;

        let quantizer = ResidualVectorQuantization::new(cfg, vb.pp("quantizer"))?;

        Ok(Self {
            acoustic_encoder,
            acoustic_decoder,
            semantic_model,
            encoder_semantic,
            decoder_semantic,
            fc,
            fc1,
            fc2,
            quantizer,
            config: cfg.clone(),
        })
    }

    /// Extract semantic features from audio using HuBERT.
    /// input_values: (B, 1, T_samples) at sample_rate (24kHz)
    /// Returns: (B, T_frames, semantic_hidden_size)
    fn extract_semantic_features(&self, input_values: &Tensor) -> Result<Tensor> {
        // Remove channel dim: (B, 1, T) -> (B, T)
        let waveform = input_values.squeeze(1)?;

        // Resample 24kHz -> 16kHz using rubato sinc interpolation
        let waveform = if self.config.sample_rate != self.config.semantic_sample_rate {
            let dev = waveform.device();
            let dtype = waveform.dtype();
            let (batch, _len) = waveform.dims2()?;
            let mut resampled_batches = Vec::with_capacity(batch);
            for bi in 0..batch {
                let samples: Vec<f32> = waveform.get(bi)?.to_dtype(DType::F32)?.to_vec1()?;
                let resampled = resample(
                    &samples,
                    self.config.sample_rate,
                    self.config.semantic_sample_rate,
                )
                .map_err(|e| candle_core::Error::Msg(format!("resample: {e}")))?;
                resampled_batches.push(
                    Tensor::from_vec(resampled.clone(), resampled.len(), dev)?
                        .to_dtype(dtype)?
                        .unsqueeze(0)?,
                );
            }
            Tensor::cat(&resampled_batches, 0)?
        } else {
            waveform
        };

        // Pad with 160 samples on each side
        let (b, _t) = waveform.dims2()?;
        let dev = waveform.device();
        let pad_left = Tensor::zeros((b, 160), waveform.dtype(), dev)?;
        let pad_right = Tensor::zeros((b, 160), waveform.dtype(), dev)?;
        let padded = Tensor::cat(&[&pad_left, &waveform, &pad_right], 1)?;

        // Forward through HuBERT, get all hidden states
        let all_hidden = self.semantic_model.forward_all_hidden_states(&padded)?;

        // Stack and mean across layers
        let stacked = Tensor::stack(&all_hidden, 1)?; // (B, num_layers+1, T_frames, H)
        let semantic_features = stacked.mean(1)?; // (B, T_frames, H)

        // Downsample by semantic_downsample_factor
        let dsf = self.config.semantic_downsample_factor();
        if dsf > 1 {
            let t_frames = semantic_features.dim(1)?;
            let indices: Vec<u32> = (0..t_frames).step_by(dsf).map(|i| i as u32).collect();
            let idx = Tensor::from_vec(indices.clone(), indices.len(), dev)?;
            Ok(semantic_features.index_select(&idx, 1)?)
        } else {
            Ok(semantic_features)
        }
    }

    /// Encode audio to discrete tokens.
    /// input_values: (B, 1, T_samples) at 24kHz
    /// Returns: audio_codes (B, num_quantizers, T_codes)
    pub fn encode(&self, input_values: &Tensor) -> Result<Tensor> {
        // Semantic path
        let semantic_features = self.extract_semantic_features(input_values)?; // (B, T, sem_H)
        let e_semantic = self.encoder_semantic.forward(
            &semantic_features.transpose(1, 2)?, // (B, sem_H, T)
        )?;
        let sem_t = e_semantic.dim(2)?;

        // Acoustic path: first try without padding, pad only if length doesn't match
        let e_acoustic = self.acoustic_encoder.forward(input_values)?;
        let ac_t = e_acoustic.dim(2)?;
        let e_acoustic = if ac_t != sem_t {
            // Try with padding
            let pad_size = self.config.hop_length() / 2;
            let (b, c, _t) = input_values.dims3()?;
            let dev = input_values.device();
            let pad_left = Tensor::zeros((b, c, pad_size), input_values.dtype(), dev)?;
            let pad_right = Tensor::zeros((b, c, pad_size), input_values.dtype(), dev)?;
            let padded_input = Tensor::cat(&[&pad_left, input_values, &pad_right], 2)?;
            let e_acoustic_padded = self.acoustic_encoder.forward(&padded_input)?;
            // Truncate to match semantic length if needed
            let ac_t2 = e_acoustic_padded.dim(2)?;
            if ac_t2 > sem_t {
                e_acoustic_padded.narrow(2, 0, sem_t)?
            } else {
                e_acoustic_padded
            }
        } else {
            e_acoustic
        };

        // Concatenate acoustic + semantic along channel dim
        let embeddings = Tensor::cat(&[&e_acoustic, &e_semantic], 1)?; // (B, hidden, T)

        // FC projection
        let embeddings = self
            .fc
            .forward(
                &embeddings.transpose(1, 2)?, // (B, T, hidden)
            )?
            .transpose(1, 2)?; // (B, hidden, T)

        // Quantize
        let audio_codes = self.quantizer.encode(&embeddings)?; // (B, num_q, T)

        Ok(audio_codes)
    }

    /// Decode discrete tokens to audio waveform.
    /// audio_codes: (B, num_quantizers, T_codes)
    /// Returns: audio_values (B, 1, T_samples)
    pub fn decode(&self, audio_codes: &Tensor) -> Result<Tensor> {
        // Transpose: (B, num_q, T) -> (num_q, B, T)
        let codes = audio_codes.permute((1, 0, 2))?;

        // Dequantize
        let quantized = self.quantizer.decode(&codes)?; // (B, hidden, T)

        // Project to acoustic dimension
        let quantized_acoustic = self
            .fc2
            .forward(
                &quantized.transpose(1, 2)?, // (B, T, hidden)
            )?
            .transpose(1, 2)?; // (B, ac_hidden, T)

        // Decode with DAC decoder
        let audio_values = self.acoustic_decoder.forward(&quantized_acoustic)?;

        Ok(audio_values)
    }
}
