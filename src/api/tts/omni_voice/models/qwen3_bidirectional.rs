//! Qwen3 backbone adapted for bidirectional attention (no causal mask, no KV cache).
//!
//! Forked from candle-transformers qwen3.rs with key modifications:
//! - Removed ConcatKvCache (full-sequence bidirectional attention each step)
//! - Removed embed_tokens (OmniVoice builds embeddings externally)
//! - Removed causal_mask() (external 4D attention mask)
//! - forward() accepts inputs_embeds + optional attention_mask
//! - Returns hidden states, no lm_head

use candle_core::{DType, Device, Module, Result, Tensor};
use candle_nn::VarBuilder;

use crate::tts::omni_voice::config::Qwen3Config;

// ---------------------------------------------------------------------------
// RMS Norm
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RmsNorm {
    weight: Tensor,
    eps: f64,
}

impl RmsNorm {
    pub fn new(size: usize, eps: f64, vb: VarBuilder) -> Result<Self> {
        let weight = vb.get(size, "weight")?;
        Ok(Self { weight, eps })
    }
}

impl Module for RmsNorm {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let dtype = x.dtype();
        let x = x.to_dtype(DType::F32)?;
        let var = x.sqr()?.mean_keepdim(candle_core::D::Minus1)?;
        let x_normed = x.broadcast_div(&(var + self.eps)?.sqrt()?)?;
        x_normed.to_dtype(dtype)?.broadcast_mul(&self.weight)
    }
}

// Re-export candle_nn::Linear and convenience constructors
pub use candle_nn::Linear;

pub fn linear(in_dim: usize, out_dim: usize, bias: bool, vb: VarBuilder) -> Result<Linear> {
    if bias {
        candle_nn::linear(in_dim, out_dim, vb)
    } else {
        candle_nn::linear_no_bias(in_dim, out_dim, vb)
    }
}

// ---------------------------------------------------------------------------
// Rotary Embedding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RotaryEmbedding {
    sin: Tensor,
    cos: Tensor,
}

impl RotaryEmbedding {
    pub fn new(dtype: DType, cfg: &Qwen3Config, dev: &Device) -> Result<Self> {
        let dim = cfg.head_dim;
        let max_seq_len = cfg.max_position_embeddings;
        let inv_freq: Vec<_> = (0..dim)
            .step_by(2)
            .map(|i| 1f32 / cfg.rope_theta.powf(i as f64 / dim as f64) as f32)
            .collect();
        let inv_freq_len = inv_freq.len();
        let inv_freq = Tensor::from_vec(inv_freq, (1, inv_freq_len), dev)?.to_dtype(DType::F32)?;
        let t = Tensor::arange(0u32, max_seq_len as u32, dev)?
            .to_dtype(DType::F32)?
            .reshape((max_seq_len, 1))?;
        let freqs = t.matmul(&inv_freq)?;
        Ok(Self {
            sin: freqs.sin()?.to_dtype(dtype)?,
            cos: freqs.cos()?.to_dtype(dtype)?,
        })
    }

    /// Apply RoPE. q, k shape: (B, H, L, D)
    pub fn apply(&self, q: &Tensor, k: &Tensor, offset: usize) -> Result<(Tensor, Tensor)> {
        let (_, _, seq_len, _) = q.dims4()?;
        let cos = self.cos.narrow(0, offset, seq_len)?;
        let sin = self.sin.narrow(0, offset, seq_len)?;
        let q_embed = candle_nn::rotary_emb::rope(&q.contiguous()?, &cos, &sin)?;
        let k_embed = candle_nn::rotary_emb::rope(&k.contiguous()?, &cos, &sin)?;
        Ok((q_embed, k_embed))
    }
}

// ---------------------------------------------------------------------------
// MLP
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Qwen3MLP {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
}

impl Qwen3MLP {
    pub fn new(cfg: &Qwen3Config, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            gate_proj: candle_nn::linear_no_bias(
                cfg.hidden_size,
                cfg.intermediate_size,
                vb.pp("gate_proj"),
            )?,
            up_proj: candle_nn::linear_no_bias(
                cfg.hidden_size,
                cfg.intermediate_size,
                vb.pp("up_proj"),
            )?,
            down_proj: candle_nn::linear_no_bias(
                cfg.intermediate_size,
                cfg.hidden_size,
                vb.pp("down_proj"),
            )?,
        })
    }
}

impl Module for Qwen3MLP {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let gate = self
            .gate_proj
            .forward(x)?
            .apply(&candle_nn::Activation::Silu)?;
        let up = self.up_proj.forward(x)?;
        (gate * up)?.apply(&self.down_proj)
    }
}

// ---------------------------------------------------------------------------
// repeat_kv for GQA
// ---------------------------------------------------------------------------

fn repeat_kv(x: Tensor, n_rep: usize) -> Result<Tensor> {
    if n_rep == 1 {
        return Ok(x);
    }
    let (b, h, l, d) = x.dims4()?;
    x.unsqueeze(2)?
        .expand((b, h, n_rep, l, d))?
        .reshape((b, h * n_rep, l, d))
}

// ---------------------------------------------------------------------------
// Attention (bidirectional, no KV cache)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Qwen3Attention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    q_norm: RmsNorm,
    k_norm: RmsNorm,
    num_heads: usize,
    num_kv_heads: usize,
    num_kv_groups: usize,
    head_dim: usize,
    hidden_size: usize,
    rotary_emb: std::sync::Arc<RotaryEmbedding>,
}

impl Qwen3Attention {
    pub fn new(
        cfg: &Qwen3Config,
        rotary_emb: std::sync::Arc<RotaryEmbedding>,
        vb: VarBuilder,
    ) -> Result<Self> {
        let head_dim = cfg.head_dim;
        let num_heads = cfg.num_attention_heads;
        let num_kv_heads = cfg.num_key_value_heads;
        let num_kv_groups = num_heads / num_kv_heads;

        let q_proj = linear(
            cfg.hidden_size,
            num_heads * head_dim,
            cfg.attention_bias,
            vb.pp("q_proj"),
        )?;
        let k_proj = linear(
            cfg.hidden_size,
            num_kv_heads * head_dim,
            cfg.attention_bias,
            vb.pp("k_proj"),
        )?;
        let v_proj = linear(
            cfg.hidden_size,
            num_kv_heads * head_dim,
            cfg.attention_bias,
            vb.pp("v_proj"),
        )?;
        let o_proj = linear(
            num_heads * head_dim,
            cfg.hidden_size,
            cfg.attention_bias,
            vb.pp("o_proj"),
        )?;

        let q_norm = RmsNorm::new(head_dim, cfg.rms_norm_eps, vb.pp("q_norm"))?;
        let k_norm = RmsNorm::new(head_dim, cfg.rms_norm_eps, vb.pp("k_norm"))?;

        let hidden_size = head_dim * num_heads;

        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            q_norm,
            k_norm,
            num_heads,
            num_kv_heads,
            num_kv_groups,
            head_dim,
            hidden_size,
            rotary_emb,
        })
    }

    pub fn forward(&self, x: &Tensor, attn_mask: Option<&Tensor>) -> Result<Tensor> {
        let (b, l, _) = x.dims3()?;

        // 1. Project
        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        // 2. Reshape: (B, L, H, D) -> (B, H, L, D)
        let q = q
            .reshape((b, l, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((b, l, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((b, l, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        // 3. Per-head RMSNorm
        let q_flat = q.flatten(0, 2)?;
        let k_flat = k.flatten(0, 2)?;
        let q_flat = self.q_norm.forward(&q_flat)?;
        let k_flat = self.k_norm.forward(&k_flat)?;
        let q = q_flat.reshape((b, self.num_heads, l, self.head_dim))?;
        let k = k_flat.reshape((b, self.num_kv_heads, l, self.head_dim))?;

        // 4. RoPE (offset=0 for bidirectional, full sequence each time)
        let (q, k) = self.rotary_emb.apply(&q, &k, 0)?;

        // 5. GQA repeat_kv
        let k = repeat_kv(k, self.num_kv_groups)?.contiguous()?;
        let v = repeat_kv(v, self.num_kv_groups)?.contiguous()?;

        // 6. Attention scores
        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let mut scores = (q.matmul(&k.transpose(2, 3)?)? * scale)?;
        if let Some(m) = attn_mask {
            scores = scores.broadcast_add(m)?;
        }
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        let ctx = probs.matmul(&v)?;

        // 7. Output projection
        ctx.transpose(1, 2)?
            .reshape((b, l, self.hidden_size))?
            .apply(&self.o_proj)
    }
}

// ---------------------------------------------------------------------------
// Decoder Layer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DecoderLayer {
    self_attn: Qwen3Attention,
    mlp: Qwen3MLP,
    ln1: RmsNorm,
    ln2: RmsNorm,
}

impl DecoderLayer {
    fn new(
        cfg: &Qwen3Config,
        rotary: std::sync::Arc<RotaryEmbedding>,
        vb: VarBuilder,
    ) -> Result<Self> {
        let self_attn = Qwen3Attention::new(cfg, rotary, vb.pp("self_attn"))?;
        let mlp = Qwen3MLP::new(cfg, vb.pp("mlp"))?;
        let ln1 = RmsNorm::new(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("input_layernorm"))?;
        let ln2 = RmsNorm::new(
            cfg.hidden_size,
            cfg.rms_norm_eps,
            vb.pp("post_attention_layernorm"),
        )?;
        Ok(Self {
            self_attn,
            mlp,
            ln1,
            ln2,
        })
    }

    fn forward(&self, x: &Tensor, mask: Option<&Tensor>) -> Result<Tensor> {
        let h = self.ln1.forward(x)?;
        let h = self.self_attn.forward(&h, mask)?;
        let x = (x + h)?;
        let h2 = self.ln2.forward(&x)?;
        let h2 = h2.apply(&self.mlp)?;
        x + h2
    }
}

// ---------------------------------------------------------------------------
// Qwen3 Bidirectional Model (returns hidden states, no embedding, no lm_head)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Qwen3Bidirectional {
    embed_tokens: candle_nn::Embedding,
    layers: Vec<DecoderLayer>,
    norm: RmsNorm,
    //device: Device,
}

impl Qwen3Bidirectional {
    pub fn new(cfg: &Qwen3Config, vb: VarBuilder) -> Result<Self> {
        let embed_tokens =
            candle_nn::embedding(cfg.vocab_size, cfg.hidden_size, vb.pp("embed_tokens"))?;
        let rotary = std::sync::Arc::new(RotaryEmbedding::new(vb.dtype(), cfg, vb.device())?);
        let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
        let vb_l = vb.pp("layers");
        for i in 0..cfg.num_hidden_layers {
            layers.push(DecoderLayer::new(cfg, rotary.clone(), vb_l.pp(i))?);
        }
        Ok(Self {
            embed_tokens,
            layers,
            norm: RmsNorm::new(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("norm"))?,
            //device: vb.device().clone(),
        })
    }

    /// Get the text embedding layer (used by OmniVoice for text token embedding).
    pub fn embed_tokens(&self) -> &candle_nn::Embedding {
        &self.embed_tokens
    }

    /*
    pub fn device(&self) -> &Device {
        &self.device
    }
    */

    /// Forward pass with pre-computed embeddings and optional attention mask.
    ///
    /// - `inputs_embeds`: (B, L, hidden_size)
    /// - `attention_mask`: optional (B, 1, L, L) float mask (0.0=attend, -inf=ignore)
    ///
    /// Returns: hidden states (B, L, hidden_size)
    pub fn forward(
        &self,
        inputs_embeds: &Tensor,
        attention_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let mut h = inputs_embeds.clone();
        for layer in &self.layers {
            h = layer.forward(&h, attention_mask)?;
        }
        self.norm.forward(&h)
    }
}
