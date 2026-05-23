//! Core OmniVoice model: audio embeddings, audio heads, iterative generation.

use anyhow::Result;
use candle_core::{DType, Device, IndexOp, Module, Tensor, D};
use candle_nn::VarBuilder;
use candle_transformers::models::deepseek2::TopKLastDimOp;

use crate::tts::omni_voice::{
    config::OmniVoiceConfig,
    models::qwen3_bidirectional::Qwen3Bidirectional,
    utils::sampling::{filter_top_k, get_time_steps, gumbel_sample},
};

/// Create an I64 tensor filled with `val`, avoiding F64 intermediates (Metal-safe).
fn full_i64(
    val: i64,
    shape: impl Into<candle_core::Shape>,
    device: &Device,
) -> candle_core::Result<Tensor> {
    let shape = shape.into();
    let n = shape.elem_count();
    Tensor::from_vec(vec![val; n], shape, device)
}

/// Create an F32 tensor filled with `val` (Metal-safe).
fn full_f32(
    val: f32,
    shape: impl Into<candle_core::Shape>,
    device: &Device,
) -> candle_core::Result<Tensor> {
    let shape = shape.into();
    let n = shape.elem_count();
    Tensor::from_vec(vec![val; n], shape, device)
}

// ---------------------------------------------------------------------------
// Generation Config
// ---------------------------------------------------------------------------

pub struct GenerationConfig {
    pub num_step: usize,
    pub guidance_scale: f64,
    pub t_shift: f64,
    pub layer_penalty_factor: f64,
    pub position_temperature: f64,
    pub class_temperature: f64,
    pub denoise: bool,
    /// Max audio duration per chunk (seconds). Used when splitting long text.
    pub audio_chunk_duration: f64,
    /// Only apply chunking when estimated audio exceeds this threshold (seconds).
    pub audio_chunk_threshold: f64,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            num_step: 32,
            guidance_scale: 2.0,
            t_shift: 0.1,
            layer_penalty_factor: 5.0,
            position_temperature: 5.0,
            class_temperature: 0.0,
            denoise: true,
            audio_chunk_duration: 15.0,
            audio_chunk_threshold: 30.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Generate Request
// ---------------------------------------------------------------------------

/// Groups the parameters for [`OmniVoice::generate`] to keep call sites clean.
pub struct GenerateRequest<'a> {
    pub tokenizer: &'a tokenizers::Tokenizer,
    pub full_text: &'a str,
    pub num_target_tokens: usize,
    pub ref_audio_tokens: Option<&'a Tensor>,
    pub ref_text: Option<&'a str>,
    pub lang: Option<&'a str>,
    pub instruct: Option<&'a str>,
    pub gen_config: &'a GenerationConfig,
    pub frame_rate: usize,
    pub speed: f64,
    pub duration_estimator: &'a crate::tts::omni_voice::utils::duration::RuleDurationEstimator,
    pub device: &'a Device,
    pub dtype: DType,
}

// ---------------------------------------------------------------------------
// OmniVoice Model
// ---------------------------------------------------------------------------

pub struct OmniVoice {
    llm: Qwen3Bidirectional,
    audio_embeddings: candle_nn::Embedding,
    audio_heads: candle_nn::Linear,
    codebook_layer_offsets: Tensor,
    config: OmniVoiceConfig,
}

impl OmniVoice {
    pub fn new(config: &OmniVoiceConfig, vb: VarBuilder) -> Result<Self> {
        let llm_vb = vb.pp("llm");
        let llm = Qwen3Bidirectional::new(&config.llm_config, llm_vb)?;

        let num_audio_tokens = config.num_audio_codebook * config.audio_vocab_size;
        let hidden_size = config.llm_config.hidden_size;

        let audio_embeddings =
            candle_nn::embedding(num_audio_tokens, hidden_size, vb.pp("audio_embeddings"))?;

        let audio_heads =
            candle_nn::linear_no_bias(hidden_size, num_audio_tokens, vb.pp("audio_heads"))?;

        let offsets: Vec<i64> = (0..config.num_audio_codebook as i64)
            .map(|i| i * config.audio_vocab_size as i64)
            .collect();
        let codebook_layer_offsets =
            Tensor::from_vec(offsets, config.num_audio_codebook, vb.device())?;

        Ok(Self {
            llm,
            audio_embeddings,
            audio_heads,
            codebook_layer_offsets,
            config: config.clone(),
        })
    }

    /// Prepare embeddings from input_ids (B, C, L) and audio_mask (B, L).
    fn prepare_embed_inputs(&self, input_ids: &Tensor, audio_mask: &Tensor) -> Result<Tensor> {
        // Text embeddings from first codebook layer
        let text_ids = input_ids.i((.., 0, ..))?; // (B, L)
        let text_embeds = self.llm.embed_tokens().forward(&text_ids)?; // (B, L, H)

        // Audio embeddings: shift IDs by codebook offsets, embed, sum across codebooks
        let audio_mask_expanded = audio_mask.unsqueeze(1)?; // (B, 1, L)
        let shifted_ids = input_ids
            .broadcast_mul(&audio_mask_expanded.to_dtype(input_ids.dtype())?)?
            .broadcast_add(&self.codebook_layer_offsets.reshape((
                1,
                self.config.num_audio_codebook,
                1,
            ))?)?;
        let shifted_ids = shifted_ids.to_dtype(DType::U32)?;

        // Embed each codebook and sum
        let (b, c, l) = shifted_ids.dims3()?;
        let flat = shifted_ids.reshape((b * c, l))?;
        let embeds = self.audio_embeddings.forward(&flat)?; // (B*C, L, H)
        let h = embeds.dim(D::Minus1)?;
        let audio_embeds = embeds.reshape((b, c, l, h))?.sum(1)?; // (B, L, H)

        // Merge: use audio_embeds where audio_mask=true, text_embeds elsewhere
        let mask = audio_mask.unsqueeze(D::Minus1)?; // (B, L, 1)
        let mask_f = mask.to_dtype(text_embeds.dtype())?;
        let result = (audio_embeds.broadcast_mul(&mask_f)?
            + text_embeds.broadcast_mul(&(Tensor::ones_like(&mask_f)? - &mask_f)?))?;
        Ok(result)
    }

    /// Forward pass: input_ids (B, C, L), audio_mask (B, L), attention_mask (B, 1, L, L).
    /// Returns logits (B, C, L, V).
    fn forward(
        &self,
        input_ids: &Tensor,
        audio_mask: &Tensor,
        attention_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let inputs_embeds = self.prepare_embed_inputs(input_ids, audio_mask)?;
        let hidden_states = self.llm.forward(&inputs_embeds, attention_mask)?;

        // Compute logits in model dtype, then convert to F32 for softmax
        let (batch_size, seq_len, _) = hidden_states.dims3()?;
        let logits_flat = self
            .audio_heads
            .forward(&hidden_states)?
            .to_dtype(DType::F32)?;

        // Reshape to (B, L, C, V) then permute to (B, C, L, V)
        let audio_logits = logits_flat
            .reshape((
                batch_size,
                seq_len,
                self.config.num_audio_codebook,
                self.config.audio_vocab_size,
            ))?
            .permute((0, 2, 1, 3))?;

        Ok(audio_logits)
    }

    /// Prepare inference inputs: build the sequence [style | text | ref_audio | target_mask].
    #[allow(clippy::too_many_arguments)]
    fn prepare_inference_inputs(
        &self,
        tokenizer: &tokenizers::Tokenizer,
        full_text: &str,
        num_target_tokens: usize,
        ref_audio_tokens: Option<&Tensor>,
        lang: Option<&str>,
        instruct: Option<&str>,
        denoise: bool,
        device: &Device,
    ) -> Result<(Tensor, Tensor)> {
        let c = self.config.num_audio_codebook;

        // Build style string
        let mut style_text = String::new();
        if denoise && ref_audio_tokens.is_some() {
            style_text.push_str("<|denoise|>");
        }
        let lang_str = lang.unwrap_or("None");
        let instruct_str = instruct.unwrap_or("None");
        style_text.push_str(&format!("<|lang_start|>{lang_str}<|lang_end|>"));
        style_text.push_str(&format!("<|instruct_start|>{instruct_str}<|instruct_end|>"));

        // Tokenize style
        let style_enc = tokenizer
            .encode(style_text.as_str(), false)
            .map_err(|e| anyhow::anyhow!("Tokenize style: {e}"))?;
        let style_ids: Vec<i64> = style_enc.get_ids().iter().map(|&id| id as i64).collect();
        let style_len = style_ids.len();
        let style_tensor = Tensor::from_vec(style_ids, (1, style_len), device)?;
        // Repeat across codebooks: (1, L) -> (1, C, L)
        let style_tokens = style_tensor
            .unsqueeze(1)?
            .expand((1, c, style_len))?
            .contiguous()?;

        // Tokenize text
        let text_str = format!("<|text_start|>{full_text}<|text_end|>");
        let text_enc = tokenizer
            .encode(text_str.as_str(), false)
            .map_err(|e| anyhow::anyhow!("Tokenize text: {e}"))?;
        let text_ids: Vec<i64> = text_enc.get_ids().iter().map(|&id| id as i64).collect();
        let text_len = text_ids.len();
        let text_tensor = Tensor::from_vec(text_ids, (1, text_len), device)?;
        let text_tokens = text_tensor
            .unsqueeze(1)?
            .expand((1, c, text_len))?
            .contiguous()?;

        // Target: all MASK
        let target_audio_tokens = full_i64(
            self.config.audio_mask_id as i64,
            (1, c, num_target_tokens),
            device,
        )?;

        // Concatenate parts
        let mut parts = vec![style_tokens, text_tokens];
        if let Some(ref_tokens) = ref_audio_tokens {
            parts.push(
                ref_tokens
                    .unsqueeze(0)?
                    .to_device(device)?
                    .to_dtype(DType::I64)?,
            );
        }
        parts.push(target_audio_tokens);

        let cond_input_ids = Tensor::cat(&parts, 2)?; // (1, C, total_len)

        // Audio mask: True for audio positions (ref_audio + target)
        let total_len = cond_input_ids.dim(2)?;
        let audio_start = total_len
            - num_target_tokens
            - ref_audio_tokens.map(|t| t.dim(1).unwrap_or(0)).unwrap_or(0);

        let mut mask_data = vec![0u8; total_len];
        mask_data[audio_start..total_len].fill(1u8);
        let audio_mask = Tensor::from_vec(mask_data, (1, total_len), device)?;

        Ok((cond_input_ids, audio_mask))
    }

    /// High-level generate: handles chunking for long texts automatically.
    ///
    /// Returns a list of chunk token tensors (each shape `(C, T)`).
    /// For short texts, returns a single-element list.
    pub fn generate(&self, req: &GenerateRequest<'_>) -> Result<Vec<Tensor>> {
        let threshold_tokens =
            (req.gen_config.audio_chunk_threshold * req.frame_rate as f64) as usize;

        if req.num_target_tokens <= threshold_tokens {
            let tokens = self.generate_iterative(
                req.tokenizer,
                req.full_text,
                req.num_target_tokens,
                req.ref_audio_tokens,
                req.lang,
                req.instruct,
                req.gen_config,
                req.device,
                req.dtype,
            )?;
            return Ok(vec![tokens]);
        }

        // Long text — split into chunks
        let avg_tokens_per_char = req.num_target_tokens as f64 / req.full_text.len().max(1) as f64;
        let text_chunk_len = (req.gen_config.audio_chunk_duration * req.frame_rate as f64
            / avg_tokens_per_char) as usize;

        let chunks = crate::tts::omni_voice::utils::text::chunk_text_punctuation(req.full_text, text_chunk_len, Some(3));

        tracing::info!("Long text: splitting into {} chunks", chunks.len());

        let has_ref = req.ref_audio_tokens.is_some();
        let mut chunk_tokens: Vec<Tensor> = Vec::with_capacity(chunks.len());

        if has_ref {
            for (ci, chunk_text) in chunks.iter().enumerate() {
                let chunk_full = crate::tts::omni_voice::utils::text::combine_text(chunk_text, req.ref_text);
                let chunk_target = self.estimate_target_tokens(
                    chunk_text,
                    req.ref_text,
                    req.ref_audio_tokens.map(|t| t.dim(1).unwrap_or(0)),
                    req.speed,
                    req.duration_estimator,
                );
                tracing::debug!("Chunk {ci}: \"{chunk_text}\" -> {chunk_target} tokens");
                let tokens = self.generate_iterative(
                    req.tokenizer,
                    &chunk_full,
                    chunk_target,
                    req.ref_audio_tokens,
                    req.lang,
                    req.instruct,
                    req.gen_config,
                    req.device,
                    req.dtype,
                )?;
                chunk_tokens.push(tokens);
            }
        } else {
            let chunk0_full = crate::tts::omni_voice::utils::text::combine_text(&chunks[0], None);
            let chunk0_target = self.estimate_target_tokens(
                &chunks[0],
                None,
                None,
                req.speed,
                req.duration_estimator,
            );
            tracing::debug!("Chunk 0: \"{}\" -> {} tokens", &chunks[0], chunk0_target);
            let first_tokens = self.generate_iterative(
                req.tokenizer,
                &chunk0_full,
                chunk0_target,
                None,
                req.lang,
                req.instruct,
                req.gen_config,
                req.device,
                req.dtype,
            )?;
            chunk_tokens.push(first_tokens);

            for (ci, chunk_text) in chunks.iter().enumerate().skip(1) {
                let ref_tok = &chunk_tokens[0];
                let chunk_full = crate::tts::omni_voice::utils::text::combine_text(chunk_text, Some(&chunks[0]));
                let chunk_target = self.estimate_target_tokens(
                    chunk_text,
                    Some(&chunks[0]),
                    Some(ref_tok.dim(1)?),
                    req.speed,
                    req.duration_estimator,
                );
                tracing::debug!("Chunk {ci}: \"{chunk_text}\" -> {chunk_target} tokens");
                let tokens = self.generate_iterative(
                    req.tokenizer,
                    &chunk_full,
                    chunk_target,
                    Some(ref_tok),
                    req.lang,
                    req.instruct,
                    req.gen_config,
                    req.device,
                    req.dtype,
                )?;
                chunk_tokens.push(tokens);
            }
        }

        Ok(chunk_tokens)
    }

    fn estimate_target_tokens(
        &self,
        text: &str,
        ref_text: Option<&str>,
        num_ref_tokens: Option<usize>,
        speed: f64,
        estimator: &crate::tts::omni_voice::utils::duration::RuleDurationEstimator,
    ) -> usize {
        let (est_ref_text, est_ref_tok) = match (ref_text, num_ref_tokens) {
            (Some(rt), Some(n)) if !rt.is_empty() && n > 0 => (rt, n as f64),
            _ => ("Nice to meet you.", 25.0),
        };
        let est = estimator.estimate_duration(text, est_ref_text, est_ref_tok);
        let est = if speed > 0.0 && speed != 1.0 {
            est / speed
        } else {
            est
        };
        est.max(1.0) as usize
    }

    /// N-step iterative unmasked decoding with classifier-free guidance (single chunk).
    #[allow(clippy::too_many_arguments)]
    fn generate_iterative(
        &self,
        tokenizer: &tokenizers::Tokenizer,
        full_text: &str,
        num_target_tokens: usize,
        ref_audio_tokens: Option<&Tensor>,
        lang: Option<&str>,
        instruct: Option<&str>,
        gen_config: &GenerationConfig,
        device: &Device,
        dtype: DType,
    ) -> Result<Tensor> {
        let c = self.config.num_audio_codebook;
        let mask_id = self.config.audio_mask_id;

        // Prepare conditional inputs
        let (cond_input_ids, cond_audio_mask) = self.prepare_inference_inputs(
            tokenizer,
            full_text,
            num_target_tokens,
            ref_audio_tokens,
            lang,
            instruct,
            gen_config.denoise,
            device,
        )?;

        let c_len = cond_input_ids.dim(2)?;
        let t_len = num_target_tokens;
        let prefix_len = c_len - t_len;
        let pad_id = mask_id as i64;

        // Padding tensor for unconditional branch (reused each iteration)
        let uncond_pad = full_i64(pad_id, (1, c, prefix_len), device)?;

        // Audio mask: cond uses full mask, uncond padded with zeros
        let uncond_audio_mask = cond_audio_mask.i((.., prefix_len..c_len))?;
        let uncond_amask_pad = Tensor::zeros((1, prefix_len), DType::U8, device)?;
        let uncond_amask_padded = Tensor::cat(&[&uncond_audio_mask, &uncond_amask_pad], 1)?;
        let batch_audio_mask = Tensor::cat(&[&cond_audio_mask, &uncond_amask_padded], 0)?;

        // Attention masks: (2, 1, c_len, c_len) float
        let minf = f32::NEG_INFINITY;
        // Conditional: full bidirectional attention within c_len
        let cond_mask = Tensor::zeros((1, 1, c_len, c_len), dtype, device)?;

        // Unconditional: attend within t_len, self-attend for padding positions
        let uncond_mask_data: Vec<f32> = (0..c_len)
            .flat_map(|i| {
                (0..c_len).map(move |j| {
                    if (i < t_len && j < t_len) || (i == j && i >= t_len) {
                        0.0
                    } else {
                        minf
                    }
                })
            })
            .collect();
        let uncond_mask =
            Tensor::from_vec(uncond_mask_data, (1, 1, c_len, c_len), device)?.to_dtype(dtype)?;

        let batch_attn_mask = Tensor::cat(&[&cond_mask, &uncond_mask], 0)?;

        // Initialize tokens: all MASK
        let mut tokens = full_i64(mask_id as i64, (1, c, t_len), device)?;

        // Compute unmasking schedule
        let timesteps = get_time_steps(0.0, 1.0, gen_config.num_step + 1, gen_config.t_shift);
        let total_mask = t_len * c;
        let mut schedule = Vec::with_capacity(gen_config.num_step);
        let mut remaining = total_mask;
        for step in 0..gen_config.num_step {
            let num = if step == gen_config.num_step - 1 {
                remaining
            } else {
                let frac = timesteps[step + 1] - timesteps[step];
                let n = (total_mask as f64 * frac).ceil() as usize;
                n.min(remaining)
            };
            schedule.push(num);
            remaining -= num;
        }

        // Layer penalty: (1, C, 1) with values layer_id * factor
        let penalty_data: Vec<f32> = (0..c)
            .map(|i| i as f32 * gen_config.layer_penalty_factor as f32)
            .collect();
        let penalty = Tensor::from_vec(penalty_data, (1, c, 1), device)?;

        // Precompute mask_id and zero tensors on CPU for per-step comparisons.
        // Shape must match flat_tokens ([total_mask]) because Candle eq/ne do not broadcast.
        let mask_id_cpu = full_i64(mask_id as i64, total_mask, &Device::Cpu)?;
        let zeros_total = Tensor::zeros(total_mask, DType::U32, &Device::Cpu)?;

        // Iterative generation loop
        for (step_idx, &k) in schedule.iter().enumerate() {
            if k == 0 {
                continue;
            }

            if step_idx % 8 == 0 || step_idx == gen_config.num_step - 1 {
                tracing::info!("Decoding step {}/{}", step_idx + 1, gen_config.num_step);
            }

            // Rebuild batch_input_ids: cond = [prefix | tokens], uncond = [tokens | pad]
            let cond_prefix = cond_input_ids.i((.., .., 0..prefix_len))?;
            let cond_row = Tensor::cat(&[&cond_prefix, &tokens], 2)?;
            let uncond_row = Tensor::cat(&[&tokens, &uncond_pad], 2)?;
            let batch_input_ids = Tensor::cat(&[&cond_row, &uncond_row], 0)?;

            // Forward pass
            let batch_logits =
                self.forward(&batch_input_ids, &batch_audio_mask, Some(&batch_attn_mask))?;

            // Extract target logits
            let c_logits = batch_logits.i((0..1, .., prefix_len..c_len, ..))?;
            let u_logits = batch_logits.i((1..2, .., 0..t_len, ..))?;

            // Predict tokens with CFG
            let (pred_tokens, confidence_scores) =
                self.predict_tokens_with_scoring(&c_logits, &u_logits, gen_config, device)?;

            // Apply layer penalty and optional Gumbel noise
            let scores = confidence_scores.broadcast_sub(&penalty)?;
            let scores = if gen_config.position_temperature > 0.0 {
                gumbel_sample(&scores, gen_config.position_temperature)?
            } else {
                scores
            };

            // --- On-device scoring and token update ---
            // Move scores/predictions to CPU for the masking + topk + scatter.
            // The data is tiny (C*T ≤ ~4000 elements) so transfer overhead is
            // negligible compared to the LLM forward pass.
            let flat_scores = scores
                .flatten_all()?
                .to_dtype(DType::F32)?
                .to_device(&Device::Cpu)?;
            let flat_pred = pred_tokens
                .flatten_all()?
                .to_dtype(DType::I64)?
                .to_device(&Device::Cpu)?;
            let flat_tokens = tokens
                .flatten_all()?
                .to_dtype(DType::I64)?
                .to_device(&Device::Cpu)?;

            // Mask out already-unmasked positions (set -inf where tokens != mask_id)
            let is_mask = flat_tokens.eq(&mask_id_cpu)?;
            let neg_inf = full_f32(f32::NEG_INFINITY, flat_scores.shape(), &Device::Cpu)?;
            let masked_scores = is_mask.where_cond(&flat_scores, &neg_inf)?;

            // Top-k positions to unmask
            let topk_out = masked_scores.topk(k)?;
            let topk_indices = topk_out.indices;

            // Scatter predicted tokens at top-k positions; keep current elsewhere
            let zeros_all = Tensor::zeros(total_mask, DType::U32, &Device::Cpu)?;
            let ones_k = Tensor::ones(k, DType::U32, &Device::Cpu)?;
            let unmask = zeros_all.scatter_add(&topk_indices, &ones_k, 0)?;
            let unmask_bool = unmask.ne(&zeros_total)?;
            let new_flat = unmask_bool.where_cond(&flat_pred, &flat_tokens)?;

            tokens = new_flat.reshape((1, c, t_len))?.to_device(device)?;
        }

        // Return (C, T) — squeeze batch dim
        Ok(tokens.squeeze(0)?)
    }

    fn predict_tokens_with_scoring(
        &self,
        c_logits: &Tensor,
        u_logits: &Tensor,
        gen_config: &GenerationConfig,
        device: &Device,
    ) -> Result<(Tensor, Tensor)> {
        let log_probs = if gen_config.guidance_scale != 0.0 {
            let c_log_probs = candle_nn::ops::log_softmax(c_logits, D::Minus1)?;
            let u_log_probs = candle_nn::ops::log_softmax(u_logits, D::Minus1)?;
            let guided =
                (&c_log_probs + ((&c_log_probs - &u_log_probs)? * gen_config.guidance_scale)?)?;
            candle_nn::ops::log_softmax(&guided, D::Minus1)?
        } else {
            candle_nn::ops::log_softmax(c_logits, D::Minus1)?
        };

        // Mask out the MASK token probability
        let mask_id = self.config.audio_mask_id;
        let vocab_size = self.config.audio_vocab_size;
        let mut mask_vec = vec![0.0f32; vocab_size];
        mask_vec[mask_id] = f32::NEG_INFINITY;
        let mask_tensor = Tensor::from_vec(mask_vec, vocab_size, device)?;
        let log_probs = log_probs.broadcast_add(&mask_tensor)?;

        let pred_tokens = if gen_config.class_temperature > 0.0 {
            // Move to CPU/F32 for stochastic sampling — Metal has known issues
            // with arg_sort, scatter, and F16 epsilon underflow in Gumbel noise.
            // Data is tiny (~C*T*vocab ≈ 500K elements), so overhead is negligible.
            let lp_cpu = log_probs.to_dtype(DType::F32)?.to_device(&Device::Cpu)?;
            let filtered = filter_top_k(&lp_cpu, 0.1)?;
            let sampled = gumbel_sample(&filtered, gen_config.class_temperature)?;
            sampled.argmax(D::Minus1)?.to_device(device)?
        } else {
            log_probs.argmax(D::Minus1)?
        };

        let confidence_scores = log_probs.max(D::Minus1)?;

        Ok((pred_tokens, confidence_scores))
    }
}
