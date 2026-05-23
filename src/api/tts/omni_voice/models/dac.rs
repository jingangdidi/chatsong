//! DAC (Descript Audio Codec) encoder and decoder for HiggsAudioV2.
//!
//! Weight key structure (from real checkpoint):
//! - acoustic_encoder.conv1.{weight,bias}
//! - acoustic_encoder.block.{N}.res_unit{1,2,3}.snake{1,2}.alpha
//! - acoustic_encoder.block.{N}.res_unit{1,2,3}.conv{1,2}.{weight,bias}
//! - acoustic_encoder.block.{N}.snake1.alpha
//! - acoustic_encoder.block.{N}.conv1.{weight,bias}
//! - acoustic_encoder.snake1.alpha
//! - acoustic_encoder.conv2.{weight,bias}
//!
//! All convolutions use fused weights (no weight_norm). Bias is present.

use candle_core::{Module, Result, Tensor, D};
use candle_nn::{Conv1d, Conv1dConfig, ConvTranspose1d, ConvTranspose1dConfig, VarBuilder};

use crate::tts::omni_voice::config::DacConfig;

// ---------------------------------------------------------------------------
// Snake1d activation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Snake1d {
    alpha: Tensor,
}

impl Snake1d {
    pub fn new(channels: usize, vb: VarBuilder) -> Result<Self> {
        let alpha = vb.get((1, channels, 1), "alpha")?;
        Ok(Self { alpha })
    }
}

impl Module for Snake1d {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let xs_shape = xs.shape();
        let xs = xs.flatten_from(2)?;
        let sin = self.alpha.broadcast_mul(&xs)?.sin()?;
        let sin = (&sin * &sin)?;
        (xs + (&self.alpha + 1e-9)?.recip()?.broadcast_mul(&sin)?)?.reshape(xs_shape)
    }
}

// ---------------------------------------------------------------------------
// ResidualUnit — keys: res_unit{N}.snake1, res_unit{N}.conv1, ...
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResidualUnit {
    snake1: Snake1d,
    conv1: Conv1d,
    snake2: Snake1d,
    conv2: Conv1d,
}

impl ResidualUnit {
    /// Create a residual unit. `vb` should point to e.g. `block.0.res_unit1`
    pub fn new(dim: usize, dilation: usize, vb: VarBuilder) -> Result<Self> {
        let pad = ((7 - 1) * dilation) / 2;
        let snake1 = Snake1d::new(dim, vb.pp("snake1"))?;
        let cfg1 = Conv1dConfig {
            dilation,
            padding: pad,
            ..Default::default()
        };
        let conv1 = candle_nn::conv1d(dim, dim, 7, cfg1, vb.pp("conv1"))?;
        let snake2 = Snake1d::new(dim, vb.pp("snake2"))?;
        let conv2 = candle_nn::conv1d(dim, dim, 1, Default::default(), vb.pp("conv2"))?;
        Ok(Self {
            snake1,
            conv1,
            snake2,
            conv2,
        })
    }
}

impl Module for ResidualUnit {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let ys = xs
            .apply(&self.snake1)?
            .apply(&self.conv1)?
            .apply(&self.snake2)?
            .apply(&self.conv2)?;
        let pad = (xs.dim(D::Minus1)? - ys.dim(D::Minus1)?) / 2;
        if pad > 0 {
            &ys + xs.narrow(D::Minus1, pad, ys.dim(D::Minus1)?)
        } else {
            ys + xs
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder Block — keys: block.{N}.res_unit{1,2,3}, block.{N}.snake1, block.{N}.conv1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EncoderBlock {
    res_unit1: ResidualUnit,
    res_unit2: ResidualUnit,
    res_unit3: ResidualUnit,
    snake1: Snake1d,
    conv1: Conv1d,
}

impl EncoderBlock {
    pub fn new(in_dim: usize, out_dim: usize, stride: usize, vb: VarBuilder) -> Result<Self> {
        let res_unit1 = ResidualUnit::new(in_dim, 1, vb.pp("res_unit1"))?;
        let res_unit2 = ResidualUnit::new(in_dim, 3, vb.pp("res_unit2"))?;
        let res_unit3 = ResidualUnit::new(in_dim, 9, vb.pp("res_unit3"))?;
        let snake1 = Snake1d::new(in_dim, vb.pp("snake1"))?;
        let cfg = Conv1dConfig {
            stride,
            padding: stride.div_ceil(2),
            ..Default::default()
        };
        let conv1 = candle_nn::conv1d(in_dim, out_dim, 2 * stride, cfg, vb.pp("conv1"))?;
        Ok(Self {
            res_unit1,
            res_unit2,
            res_unit3,
            snake1,
            conv1,
        })
    }
}

impl Module for EncoderBlock {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        xs.apply(&self.res_unit1)?
            .apply(&self.res_unit2)?
            .apply(&self.res_unit3)?
            .apply(&self.snake1)?
            .apply(&self.conv1)
    }
}

// ---------------------------------------------------------------------------
// Encoder — keys: conv1, block.{0..N}, snake1, conv2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DacEncoder {
    conv1: Conv1d,
    blocks: Vec<EncoderBlock>,
    snake1: Snake1d,
    conv2: Conv1d,
}

impl DacEncoder {
    pub fn new(cfg: &DacConfig, vb: VarBuilder) -> Result<Self> {
        let strides = &cfg.downsampling_ratios;
        let mut d_model = cfg.encoder_hidden_size;
        let d_latent = cfg.hidden_size;

        let conv1_cfg = Conv1dConfig {
            padding: 3,
            ..Default::default()
        };
        let conv1 = candle_nn::conv1d(1, d_model, 7, conv1_cfg, vb.pp("conv1"))?;

        let mut blocks = Vec::with_capacity(strides.len());
        for (idx, &stride) in strides.iter().enumerate() {
            let out_dim = d_model * 2;
            blocks.push(EncoderBlock::new(
                d_model,
                out_dim,
                stride,
                vb.pp("block").pp(idx),
            )?);
            d_model = out_dim;
        }

        let snake1 = Snake1d::new(d_model, vb.pp("snake1"))?;
        let conv2_cfg = Conv1dConfig {
            padding: 1,
            ..Default::default()
        };
        let conv2 = candle_nn::conv1d(d_model, d_latent, 3, conv2_cfg, vb.pp("conv2"))?;

        Ok(Self {
            conv1,
            blocks,
            snake1,
            conv2,
        })
    }
}

impl Module for DacEncoder {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut xs = xs.apply(&self.conv1)?;
        for block in &self.blocks {
            xs = xs.apply(block)?;
        }
        xs.apply(&self.snake1)?.apply(&self.conv2)
    }
}

// ---------------------------------------------------------------------------
// Decoder Block — keys: block.{N}.snake1, block.{N}.conv_t1, block.{N}.res_unit{1,2,3}
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecoderBlock {
    snake1: Snake1d,
    conv_t1: ConvTranspose1d,
    res_unit1: ResidualUnit,
    res_unit2: ResidualUnit,
    res_unit3: ResidualUnit,
}

impl DecoderBlock {
    pub fn new(in_dim: usize, out_dim: usize, stride: usize, vb: VarBuilder) -> Result<Self> {
        let snake1 = Snake1d::new(in_dim, vb.pp("snake1"))?;
        let cfg = ConvTranspose1dConfig {
            stride,
            padding: stride.div_ceil(2),
            output_padding: stride % 2,
            ..Default::default()
        };
        let conv_t1 =
            candle_nn::conv_transpose1d(in_dim, out_dim, 2 * stride, cfg, vb.pp("conv_t1"))?;
        let res_unit1 = ResidualUnit::new(out_dim, 1, vb.pp("res_unit1"))?;
        let res_unit2 = ResidualUnit::new(out_dim, 3, vb.pp("res_unit2"))?;
        let res_unit3 = ResidualUnit::new(out_dim, 9, vb.pp("res_unit3"))?;
        Ok(Self {
            snake1,
            conv_t1,
            res_unit1,
            res_unit2,
            res_unit3,
        })
    }
}

impl Module for DecoderBlock {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        xs.apply(&self.snake1)?
            .apply(&self.conv_t1)?
            .apply(&self.res_unit1)?
            .apply(&self.res_unit2)?
            .apply(&self.res_unit3)
    }
}

// ---------------------------------------------------------------------------
// Decoder — keys: conv1, block.{0..N}, snake1, conv2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DacDecoder {
    conv1: Conv1d,
    blocks: Vec<DecoderBlock>,
    snake1: Snake1d,
    conv2: Conv1d,
}

impl DacDecoder {
    pub fn new(cfg: &DacConfig, vb: VarBuilder) -> Result<Self> {
        let rates = &cfg.upsampling_ratios;
        let in_c = cfg.hidden_size;
        let mut channels = cfg.decoder_hidden_size;

        let conv1_cfg = Conv1dConfig {
            padding: 3,
            ..Default::default()
        };
        let conv1 = candle_nn::conv1d(in_c, channels, 7, conv1_cfg, vb.pp("conv1"))?;

        let mut blocks = Vec::with_capacity(rates.len());
        for (idx, &stride) in rates.iter().enumerate() {
            let out_dim = channels / 2;
            blocks.push(DecoderBlock::new(
                channels,
                out_dim,
                stride,
                vb.pp("block").pp(idx),
            )?);
            channels = out_dim;
        }

        let snake1 = Snake1d::new(channels, vb.pp("snake1"))?;
        let conv2 = candle_nn::conv1d(channels, 1, 7, conv1_cfg, vb.pp("conv2"))?;

        Ok(Self {
            conv1,
            blocks,
            snake1,
            conv2,
        })
    }
}

impl Module for DacDecoder {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut xs = xs.apply(&self.conv1)?;
        for block in &self.blocks {
            xs = xs.apply(block)?;
        }
        // No tanh — removed for HiggsAudioV2
        xs.apply(&self.snake1)?.apply(&self.conv2)
    }
}
