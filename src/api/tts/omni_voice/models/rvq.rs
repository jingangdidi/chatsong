//! Residual Vector Quantization with Euclidean codebook (HiggsAudioV2 style).
//!
//! Unlike the candle DAC VQ (Conv1d projections), HiggsAudioV2 uses nn.Linear
//! for project_in/out and Euclidean distance for codebook lookup.

use candle_core::{IndexOp, Module, Result, Tensor, D};
use candle_nn::VarBuilder;

use crate::tts::omni_voice::config::HiggsAudioV2Config;

// ---------------------------------------------------------------------------
// Euclidean Codebook
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EuclideanCodebook {
    embed: Tensor, // (codebook_size, codebook_dim)
}

impl EuclideanCodebook {
    pub fn new(codebook_size: usize, codebook_dim: usize, vb: VarBuilder) -> Result<Self> {
        let embed = vb.get((codebook_size, codebook_dim), "embed")?;
        Ok(Self { embed })
    }

    /// Quantize: find nearest codebook entry for each vector.
    /// hidden_states: (N, D) -> returns indices (N,)
    pub fn quantize(&self, hidden_states: &Tensor) -> Result<Tensor> {
        let embed_t = self.embed.t()?; // (D, codebook_size)
        let scaled = hidden_states.sqr()?.sum_keepdim(1)?; // (N, 1)
        let embed_sq = embed_t.sqr()?.sum_keepdim(0)?; // (1, codebook_size)
        let cross = hidden_states.matmul(&embed_t)?; // (N, codebook_size)
                                                     // dist = ||x||^2 - 2*x@e + ||e||^2  (squared Euclidean distance)
        let cross_2 = (cross * 2.0)?;
        let dist = scaled.broadcast_sub(&cross_2)?.broadcast_add(&embed_sq)?;
        // argmin of distance
        let neg_dist = dist.neg()?;
        neg_dist.argmax(D::Minus1)
    }

    /// Encode: flatten input, quantize, reshape back.
    /// hidden_states: (..., D) -> indices (...,)
    pub fn encode(&self, hidden_states: &Tensor) -> Result<Tensor> {
        let shape = hidden_states.shape().clone();
        let dims = shape.dims();
        let d = dims[dims.len() - 1];
        let flat = hidden_states.reshape(((), d))?;
        let indices = self.quantize(&flat)?;
        let out_shape: Vec<usize> = dims[..dims.len() - 1].to_vec();
        indices.reshape(out_shape)
    }

    /// Decode: embedding lookup.
    pub fn decode(&self, indices: &Tensor) -> Result<Tensor> {
        // F.embedding(indices, self.embed)
        indices.apply(&candle_nn::Embedding::new(
            self.embed.clone(),
            self.embed.dim(1)?,
        ))
    }
}

// ---------------------------------------------------------------------------
// Vector Quantization (single codebook with Linear projections)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VectorQuantization {
    codebook: EuclideanCodebook,
    project_in: candle_nn::Linear,
    project_out: candle_nn::Linear,
}

impl VectorQuantization {
    pub fn new(
        hidden_size: usize,
        codebook_size: usize,
        codebook_dim: usize,
        vb: VarBuilder,
    ) -> Result<Self> {
        let codebook = EuclideanCodebook::new(codebook_size, codebook_dim, vb.pp("codebook"))?;
        let project_in = candle_nn::linear(hidden_size, codebook_dim, vb.pp("project_in"))?;
        let project_out = candle_nn::linear(codebook_dim, hidden_size, vb.pp("project_out"))?;
        Ok(Self {
            codebook,
            project_in,
            project_out,
        })
    }

    /// Encode: (B, C, T) -> indices (B, T)
    pub fn encode(&self, hidden_states: &Tensor) -> Result<Tensor> {
        let x = hidden_states.permute((0, 2, 1))?; // (B, T, C)
        let x = self.project_in.forward(&x)?; // (B, T, codebook_dim)
        self.codebook.encode(&x)
    }

    /// Decode: indices (B, T) -> (B, C, T)
    pub fn decode(&self, indices: &Tensor) -> Result<Tensor> {
        let quantized = self.codebook.decode(indices)?; // (B, T, codebook_dim)
        let quantized = self.project_out.forward(&quantized)?; // (B, T, hidden_size)
        quantized.permute((0, 2, 1)) // (B, hidden_size, T)
    }
}

// ---------------------------------------------------------------------------
// Residual Vector Quantization
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResidualVectorQuantization {
    quantizers: Vec<VectorQuantization>,
}

impl ResidualVectorQuantization {
    pub fn new(cfg: &HiggsAudioV2Config, vb: VarBuilder) -> Result<Self> {
        let n = cfg.num_quantizers();
        let hidden = cfg.hidden_size();
        let cb_size = cfg.codebook_size;
        let cb_dim = cfg.codebook_dim;

        let mut quantizers = Vec::with_capacity(n);
        for i in 0..n {
            quantizers.push(VectorQuantization::new(
                hidden,
                cb_size,
                cb_dim,
                vb.pp("quantizers").pp(i),
            )?);
        }
        Ok(Self { quantizers })
    }

    /// Encode: embeddings (B, hidden, T) -> codes (B, num_quantizers, T)
    pub fn encode(&self, embeddings: &Tensor) -> Result<Tensor> {
        let mut residual = embeddings.clone();
        let mut all_indices = Vec::with_capacity(self.quantizers.len());
        for q in &self.quantizers {
            let indices = q.encode(&residual)?; // (B, T)
            let quantized = q.decode(&indices)?; // (B, hidden, T)
            residual = (&residual - &quantized)?;
            all_indices.push(indices.unsqueeze(1)?); // (B, 1, T)
        }
        Tensor::cat(&all_indices, 1) // (B, num_quantizers, T)
    }

    /// Decode: codes (num_quantizers, B, T) -> embeddings (B, hidden, T)
    pub fn decode(&self, codes: &Tensor) -> Result<Tensor> {
        // Clamp to available quantizers to prevent index-out-of-bounds
        let n = codes.dim(0)?.min(self.quantizers.len());
        let mut out: Option<Tensor> = None;
        for i in 0..n {
            let indices = codes.i(i)?; // (B, T)
            let quantized = self.quantizers[i].decode(&indices)?; // (B, hidden, T)
            out = Some(match out {
                Some(acc) => (acc + quantized)?,
                None => quantized,
            });
        }
        out.ok_or_else(|| candle_core::Error::Msg("empty codebooks".to_string()))
    }
}
