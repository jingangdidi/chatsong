//! HuBERT model for semantic feature extraction.
//!
//! Weight key structure (from real checkpoint):
//! - semantic_model.feature_extractor.conv_layers.{0-6}.conv.weight
//! - semantic_model.feature_extractor.conv_layers.0.layer_norm.{weight,bias}
//! - semantic_model.feature_projection.layer_norm.{weight,bias}  (dim=512)
//! - semantic_model.feature_projection.projection.{weight,bias}  (512->768)
//! - semantic_model.encoder.pos_conv_embed.conv.bias
//! - semantic_model.encoder.pos_conv_embed.conv.parametrizations.weight.original0  (weight_g)
//! - semantic_model.encoder.pos_conv_embed.conv.parametrizations.weight.original1  (weight_v)
//! - semantic_model.encoder.layers.{0-11}.attention.{q,k,v,out}_proj.{weight,bias}
//! - semantic_model.encoder.layers.{0-11}.feed_forward.intermediate_dense.{weight,bias}
//! - semantic_model.encoder.layers.{0-11}.feed_forward.output_dense.{weight,bias}
//! - semantic_model.encoder.layers.{0-11}.layer_norm.{weight,bias}
//! - semantic_model.encoder.layers.{0-11}.final_layer_norm.{weight,bias}
//! - semantic_model.encoder.layer_norm.{weight,bias}

use candle_core::{Module, Result, Tensor};
use candle_nn::{Conv1d, Conv1dConfig, VarBuilder};

use crate::tts::omni_voice::config::HuBERTConfig;

// ---------------------------------------------------------------------------
// Feature Extractor: 7 Conv1d layers
// Layer 0 has GroupNorm (loaded as layer_norm); layers 1-6 have only conv.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ConvLayer {
    conv: Conv1d,
    layer_norm: Option<candle_nn::LayerNorm>,
}

impl ConvLayer {
    fn new(
        in_c: usize,
        out_c: usize,
        kernel: usize,
        stride: usize,
        use_group_norm: bool,
        bias: bool,
        vb: VarBuilder,
    ) -> Result<Self> {
        let cfg = Conv1dConfig {
            stride,
            ..Default::default()
        };
        let conv = if bias {
            candle_nn::conv1d(in_c, out_c, kernel, cfg, vb.pp("conv"))?
        } else {
            candle_nn::conv1d_no_bias(in_c, out_c, kernel, cfg, vb.pp("conv"))?
        };
        let layer_norm = if use_group_norm {
            Some(candle_nn::layer_norm(out_c, 1e-5, vb.pp("layer_norm"))?)
        } else {
            None
        };
        Ok(Self { conv, layer_norm })
    }
}

impl Module for ConvLayer {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let h = xs.apply(&self.conv)?;
        let h = if let Some(ln) = &self.layer_norm {
            // layer_norm expects (..., C) but conv output is (B, C, T)
            let h = h.transpose(1, 2)?;
            let h = ln.forward(&h)?;
            h.transpose(1, 2)?
        } else {
            h
        };
        h.gelu()
    }
}

#[derive(Debug, Clone)]
struct FeatureExtractor {
    layers: Vec<ConvLayer>,
}

impl FeatureExtractor {
    fn new(cfg: &HuBERTConfig, vb: VarBuilder) -> Result<Self> {
        let n_layers = cfg.conv_dim.len();
        let use_group_norm = cfg.feat_extract_norm == "group";
        let mut layers = Vec::with_capacity(n_layers);

        for i in 0..n_layers {
            let in_c = if i == 0 { 1 } else { cfg.conv_dim[i - 1] };
            let out_c = cfg.conv_dim[i];
            let kernel = cfg.conv_kernel[i];
            let stride = cfg.conv_stride[i];
            // Group norm only on first layer
            let gn = use_group_norm && i == 0;
            layers.push(ConvLayer::new(
                in_c,
                out_c,
                kernel,
                stride,
                gn,
                cfg.conv_bias,
                vb.pp("conv_layers").pp(i),
            )?);
        }
        Ok(Self { layers })
    }
}

impl Module for FeatureExtractor {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut h = xs.clone();
        for layer in &self.layers {
            h = layer.forward(&h)?;
        }
        Ok(h)
    }
}

// ---------------------------------------------------------------------------
// Feature Projection: LayerNorm(feat_dim) + Linear(feat_dim -> hidden)
// Note: layer_norm operates on feat_dim (512), NOT hidden_size (768)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FeatureProjection {
    layer_norm: candle_nn::LayerNorm,
    projection: candle_nn::Linear,
}

impl FeatureProjection {
    fn new(feat_dim: usize, hidden: usize, eps: f64, vb: VarBuilder) -> Result<Self> {
        let layer_norm = candle_nn::layer_norm(feat_dim, eps, vb.pp("layer_norm"))?;
        let projection = candle_nn::linear(feat_dim, hidden, vb.pp("projection"))?;
        Ok(Self {
            layer_norm,
            projection,
        })
    }
}

impl Module for FeatureProjection {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let h = self.layer_norm.forward(xs)?;
        self.projection.forward(&h)
    }
}

// ---------------------------------------------------------------------------
// Positional Conv Embedding: grouped Conv1d with PARAMETRIZED weight norm
//
// Real keys:
//   encoder.pos_conv_embed.conv.bias: [768]
//   encoder.pos_conv_embed.conv.parametrizations.weight.original0: [1, 1, 128]  (weight_g)
//   encoder.pos_conv_embed.conv.parametrizations.weight.original1: [768, 48, 128]  (weight_v)
//
// Fused weight = weight_g * weight_v / ||weight_v||
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PositionalConvEmbedding {
    weight: Tensor,
    bias: Tensor,
    padding: usize,
    groups: usize,
    /// Number of trailing frames to remove (1 for even kernel, 0 for odd).
    num_pad_remove: usize,
}

impl PositionalConvEmbedding {
    fn new(hidden: usize, kernel: usize, groups: usize, vb: VarBuilder) -> Result<Self> {
        let padding = kernel / 2;
        let conv_vb = vb.pp("conv");

        // Load parametrized weight norm tensors
        let param_vb = conv_vb.pp("parametrizations").pp("weight");
        let weight_g = param_vb.get((1, 1, kernel), "original0")?; // (1, 1, K)
        let weight_v = param_vb.get((hidden, hidden / groups, kernel), "original1")?; // (C_out, C_in/groups, K)

        // Fuse: weight = weight_g * weight_v / ||weight_v||
        // ||weight_v|| computed per output channel
        let v_flat = weight_v.flatten(1, 2)?; // (C_out, C_in/groups * K)
        let v_norm = v_flat.sqr()?.sum_keepdim(1)?.sqrt()?; // (C_out, 1)
        let v_norm = v_norm.unsqueeze(2)?; // (C_out, 1, 1)
        let weight = (weight_v.broadcast_div(&v_norm)?.broadcast_mul(&weight_g))?;

        let bias = conv_vb.get(hidden, "bias")?;

        // HubertSamePadLayer: remove 1 trailing frame for even kernel sizes
        let num_pad_remove = if kernel.is_multiple_of(2) { 1 } else { 0 };

        Ok(Self {
            weight,
            bias,
            padding,
            groups,
            num_pad_remove,
        })
    }
}

impl Module for PositionalConvEmbedding {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        // xs: (B, T, C) -> (B, C, T)
        let h = xs.transpose(1, 2)?;
        let h = h.conv1d(
            &self.weight,
            self.padding,
            1, // stride
            1, // dilation
            self.groups,
        )?;
        let h = h.broadcast_add(&self.bias.unsqueeze(0)?.unsqueeze(2)?)?;
        // SamePadLayer: trim trailing frame for even kernel (kernel=128 → remove 1)
        let h = if self.num_pad_remove > 0 {
            let t = h.dim(2)?;
            h.narrow(2, 0, t - self.num_pad_remove)?
        } else {
            h
        };
        // (B, C, T) -> (B, T, C)
        let h = h.transpose(1, 2)?;
        h.gelu()
    }
}

// ---------------------------------------------------------------------------
// Self-Attention
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SelfAttention {
    q_proj: candle_nn::Linear,
    k_proj: candle_nn::Linear,
    v_proj: candle_nn::Linear,
    out_proj: candle_nn::Linear,
    num_heads: usize,
    head_dim: usize,
}

impl SelfAttention {
    fn new(hidden: usize, num_heads: usize, vb: VarBuilder) -> Result<Self> {
        let head_dim = hidden / num_heads;
        let q_proj = candle_nn::linear(hidden, hidden, vb.pp("q_proj"))?;
        let k_proj = candle_nn::linear(hidden, hidden, vb.pp("k_proj"))?;
        let v_proj = candle_nn::linear(hidden, hidden, vb.pp("v_proj"))?;
        let out_proj = candle_nn::linear(hidden, hidden, vb.pp("out_proj"))?;
        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            out_proj,
            num_heads,
            head_dim,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (b, l, _) = xs.dims3()?;
        let q = self
            .q_proj
            .forward(xs)?
            .reshape((b, l, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = self
            .k_proj
            .forward(xs)?
            .reshape((b, l, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = self
            .v_proj
            .forward(xs)?
            .reshape((b, l, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;

        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let scores = (q.matmul(&k.transpose(2, 3)?)? * scale)?;
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        let ctx = probs.matmul(&v)?;
        let ctx = ctx
            .transpose(1, 2)?
            .reshape((b, l, self.num_heads * self.head_dim))?;
        self.out_proj.forward(&ctx)
    }
}

// ---------------------------------------------------------------------------
// Feed Forward
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FeedForward {
    intermediate_dense: candle_nn::Linear,
    output_dense: candle_nn::Linear,
}

impl FeedForward {
    fn new(hidden: usize, intermediate: usize, vb: VarBuilder) -> Result<Self> {
        let intermediate_dense =
            candle_nn::linear(hidden, intermediate, vb.pp("intermediate_dense"))?;
        let output_dense = candle_nn::linear(intermediate, hidden, vb.pp("output_dense"))?;
        Ok(Self {
            intermediate_dense,
            output_dense,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let h = self.intermediate_dense.forward(xs)?.gelu()?;
        self.output_dense.forward(&h)
    }
}

// ---------------------------------------------------------------------------
// Encoder Layer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct EncoderLayer {
    attention: SelfAttention,
    feed_forward: FeedForward,
    layer_norm: candle_nn::LayerNorm,
    final_layer_norm: candle_nn::LayerNorm,
}

impl EncoderLayer {
    fn new(cfg: &HuBERTConfig, vb: VarBuilder) -> Result<Self> {
        let attention =
            SelfAttention::new(cfg.hidden_size, cfg.num_attention_heads, vb.pp("attention"))?;
        let feed_forward = FeedForward::new(
            cfg.hidden_size,
            cfg.intermediate_size,
            vb.pp("feed_forward"),
        )?;
        let layer_norm =
            candle_nn::layer_norm(cfg.hidden_size, cfg.layer_norm_eps, vb.pp("layer_norm"))?;
        let final_layer_norm = candle_nn::layer_norm(
            cfg.hidden_size,
            cfg.layer_norm_eps,
            vb.pp("final_layer_norm"),
        )?;
        Ok(Self {
            attention,
            feed_forward,
            layer_norm,
            final_layer_norm,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        // POST-norm (matches Python HubertEncoderLayer):
        // h = layer_norm(residual + attention(h))
        // h = final_layer_norm(h + feed_forward(h))
        let attn_out = self.attention.forward(xs)?;
        let h = (xs + attn_out)?;
        let h = self.layer_norm.forward(&h)?;
        let ff_out = self.feed_forward.forward(&h)?;
        let h = (h + ff_out)?;
        self.final_layer_norm.forward(&h)
    }
}

// ---------------------------------------------------------------------------
// HuBERT Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HuBERTModel {
    feature_extractor: FeatureExtractor,
    feature_projection: FeatureProjection,
    pos_conv_embed: PositionalConvEmbedding,
    encoder_layers: Vec<EncoderLayer>,
    encoder_layer_norm: candle_nn::LayerNorm,
}

impl HuBERTModel {
    pub fn new(cfg: &HuBERTConfig, vb: VarBuilder) -> Result<Self> {
        let feat_dim = *cfg.conv_dim.last().unwrap_or(&512);
        let feature_extractor = FeatureExtractor::new(cfg, vb.pp("feature_extractor"))?;
        // Note: layer_norm in feature_projection operates on feat_dim (512), not hidden_size
        let feature_projection = FeatureProjection::new(
            feat_dim,
            cfg.hidden_size,
            cfg.layer_norm_eps,
            vb.pp("feature_projection"),
        )?;
        let pos_conv_embed = PositionalConvEmbedding::new(
            cfg.hidden_size,
            cfg.num_conv_pos_embeddings,
            cfg.num_conv_pos_embedding_groups,
            vb.pp("encoder").pp("pos_conv_embed"),
        )?;
        let mut encoder_layers = Vec::with_capacity(cfg.num_hidden_layers);
        for i in 0..cfg.num_hidden_layers {
            encoder_layers.push(EncoderLayer::new(cfg, vb.pp("encoder").pp("layers").pp(i))?);
        }
        let encoder_layer_norm = candle_nn::layer_norm(
            cfg.hidden_size,
            cfg.layer_norm_eps,
            vb.pp("encoder").pp("layer_norm"),
        )?;
        Ok(Self {
            feature_extractor,
            feature_projection,
            pos_conv_embed,
            encoder_layers,
            encoder_layer_norm,
        })
    }

    /// Forward pass returning all hidden states (embedding + each layer output).
    /// input: (B, T_samples) raw waveform at 16kHz
    /// Returns: Vec of (B, T_frames, hidden_size), length = 1 + num_layers
    pub fn forward_all_hidden_states(&self, input_values: &Tensor) -> Result<Vec<Tensor>> {
        // Feature extraction: (B, T) -> (B, 1, T) -> (B, C, T_frames) -> (B, T_frames, C)
        let xs = input_values.unsqueeze(1)?;
        let features = self.feature_extractor.forward(&xs)?;
        let features = features.transpose(1, 2)?; // (B, T_frames, feat_dim)

        // Feature projection: LayerNorm(feat_dim) then Linear(feat_dim -> hidden)
        let hidden = self.feature_projection.forward(&features)?;

        // Positional encoding + layer norm (matches Python HubertEncoder.forward)
        let pos = self.pos_conv_embed.forward(&hidden)?;
        let mut h = (hidden + pos)?;
        h = self.encoder_layer_norm.forward(&h)?;
        // dropout omitted (inference mode)

        let mut all_hidden = Vec::with_capacity(self.encoder_layers.len() + 1);
        all_hidden.push(h.clone());

        // Encoder layers
        for layer in &self.encoder_layers {
            h = layer.forward(&h)?;
            all_hidden.push(h.clone());
        }

        Ok(all_hidden)
    }
}
