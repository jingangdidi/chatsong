//! Semantic encoder/decoder for HiggsAudioV2 tokenizer.
//!
//! Small convolutional networks that process HuBERT semantic features.

use candle_core::{Module, Result, Tensor};
use candle_nn::{Conv1d, Conv1dConfig, ConvTranspose1d, ConvTranspose1dConfig, VarBuilder};

use crate::tts::omni_voice::config::HiggsAudioV2Config;

// ---------------------------------------------------------------------------
// Residual Unit (ELU-based)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ResidualUnit {
    conv1: Conv1d,
    conv2: Conv1d,
}

fn elu(xs: &Tensor) -> Result<Tensor> {
    let pos = xs.relu()?;
    let neg = (xs.exp()? - 1.0)?.minimum(0.0)?;
    pos + neg
}

impl ResidualUnit {
    fn new(
        in_channels: usize,
        out_channels: usize,
        dilation: usize,
        kernel_size: usize,
        vb: VarBuilder,
    ) -> Result<Self> {
        let padding = ((kernel_size - 1) / 2) * dilation;
        let cfg1 = Conv1dConfig {
            dilation,
            padding,
            ..Default::default()
        };
        // No bias
        let conv1 = candle_nn::conv1d_no_bias(
            in_channels,
            out_channels,
            kernel_size,
            cfg1,
            vb.pp("conv1"),
        )?;
        let conv2 = candle_nn::conv1d_no_bias(
            out_channels,
            out_channels,
            1,
            Default::default(),
            vb.pp("conv2"),
        )?;
        Ok(Self { conv1, conv2 })
    }
}

impl Module for ResidualUnit {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let h = elu(xs)?;
        let h = h.apply(&self.conv1)?;
        let h = elu(&h)?;
        let h = h.apply(&self.conv2)?;
        xs + h
    }
}

// ---------------------------------------------------------------------------
// Semantic Encoder Block
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SemanticEncoderBlock {
    res_units: Vec<ResidualUnit>,
    conv: Conv1d,
}

impl SemanticEncoderBlock {
    fn new(
        in_channels: usize,
        out_channels: usize,
        stride: usize,
        dilations: &[usize],
        unit_kernel_size: usize,
        vb: VarBuilder,
    ) -> Result<Self> {
        let mut res_units = Vec::with_capacity(dilations.len());
        for (i, &d) in dilations.iter().enumerate() {
            res_units.push(ResidualUnit::new(
                in_channels,
                in_channels,
                d,
                unit_kernel_size,
                vb.pp("res_units").pp(i),
            )?);
        }

        let (kernel, padding) = if stride == 1 {
            (3, 1)
        } else {
            (2 * stride, (2 * stride - 1) / 2)
        };
        let cfg = Conv1dConfig {
            stride,
            padding,
            ..Default::default()
        };
        let conv = candle_nn::conv1d(in_channels, out_channels, kernel, cfg, vb.pp("conv"))?;

        Ok(Self { res_units, conv })
    }
}

impl Module for SemanticEncoderBlock {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut h = xs.clone();
        for unit in &self.res_units {
            h = unit.forward(&h)?;
        }
        h.apply(&self.conv)
    }
}

// ---------------------------------------------------------------------------
// Semantic Encoder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SemanticEncoder {
    conv: Conv1d,
    conv_blocks: Vec<SemanticEncoderBlock>,
}

impl SemanticEncoder {
    pub fn new(cfg: &HiggsAudioV2Config, vb: VarBuilder) -> Result<Self> {
        let sem_hidden = cfg.semantic_hidden_size();
        let k = cfg.kernel_size;
        let conv = candle_nn::conv1d_no_bias(
            sem_hidden,
            sem_hidden,
            k,
            Conv1dConfig {
                padding: k / 2,
                ..Default::default()
            },
            vb.pp("conv"),
        )?;

        let mut in_ch = sem_hidden;
        let mut blocks = Vec::new();
        for (i, &stride) in cfg.strides.iter().enumerate() {
            let out_ch = (sem_hidden as f64 * cfg.channel_ratios[i]) as usize;
            blocks.push(SemanticEncoderBlock::new(
                in_ch,
                out_ch,
                stride,
                &cfg.block_dilations,
                cfg.unit_kernel_size,
                vb.pp("conv_blocks").pp(i),
            )?);
            in_ch = out_ch;
        }

        Ok(Self {
            conv,
            conv_blocks: blocks,
        })
    }
}

impl Module for SemanticEncoder {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut h = xs.apply(&self.conv)?;
        for block in &self.conv_blocks {
            h = block.forward(&h)?;
        }
        Ok(h)
    }
}

// ---------------------------------------------------------------------------
// Semantic Decoder Block
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SemanticDecoderBlock {
    conv: Conv1dOrTranspose,
    res_units: Vec<ResidualUnit>,
}

#[derive(Debug, Clone)]
enum Conv1dOrTranspose {
    Conv(Conv1d),
    Transpose(ConvTranspose1d),
}

impl Module for Conv1dOrTranspose {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        match self {
            Conv1dOrTranspose::Conv(c) => xs.apply(c),
            Conv1dOrTranspose::Transpose(c) => xs.apply(c),
        }
    }
}

impl SemanticDecoderBlock {
    fn new(
        in_channels: usize,
        out_channels: usize,
        stride: usize,
        dilations: &[usize],
        unit_kernel_size: usize,
        vb: VarBuilder,
    ) -> Result<Self> {
        let conv = if stride == 1 {
            let cfg = Conv1dConfig {
                padding: 1,
                ..Default::default()
            };
            Conv1dOrTranspose::Conv(candle_nn::conv1d(
                in_channels,
                out_channels,
                3,
                cfg,
                vb.pp("conv"),
            )?)
        } else {
            let kernel_size = 2 * stride;
            let padding = stride.div_ceil(2);
            let output_padding = if stride % 2 == 1 { 1 } else { 0 };
            let cfg = ConvTranspose1dConfig {
                stride,
                padding,
                output_padding,
                ..Default::default()
            };
            Conv1dOrTranspose::Transpose(candle_nn::conv_transpose1d_no_bias(
                in_channels,
                out_channels,
                kernel_size,
                cfg,
                vb.pp("conv"),
            )?)
        };

        let mut res_units = Vec::with_capacity(dilations.len());
        for (i, &d) in dilations.iter().enumerate() {
            res_units.push(ResidualUnit::new(
                out_channels,
                out_channels,
                d,
                unit_kernel_size,
                vb.pp("res_units").pp(i),
            )?);
        }

        Ok(Self { conv, res_units })
    }
}

impl Module for SemanticDecoderBlock {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut h = self.conv.forward(xs)?;
        for unit in &self.res_units {
            h = unit.forward(&h)?;
        }
        Ok(h)
    }
}

// ---------------------------------------------------------------------------
// Semantic Decoder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SemanticDecoder {
    conv1: Conv1d,
    conv_blocks: Vec<SemanticDecoderBlock>,
    conv2: Conv1d,
}

impl SemanticDecoder {
    pub fn new(cfg: &HiggsAudioV2Config, vb: VarBuilder) -> Result<Self> {
        let sem_hidden = cfg.semantic_hidden_size();
        let k = cfg.kernel_size;
        let first_ratio = cfg.channel_ratios[0];
        let first_out = (sem_hidden as f64 * first_ratio) as usize;

        let conv1 = candle_nn::conv1d_no_bias(
            sem_hidden,
            first_out,
            k,
            Conv1dConfig {
                padding: k / 2,
                ..Default::default()
            },
            vb.pp("conv1"),
        )?;

        let mut blocks = Vec::new();
        for (i, &stride) in cfg.strides.iter().enumerate() {
            let in_ch = (sem_hidden as f64 * cfg.channel_ratios[i]) as usize;
            let out_ch = if i + 1 < cfg.channel_ratios.len() {
                (sem_hidden as f64 * cfg.channel_ratios[i + 1]) as usize
            } else {
                sem_hidden
            };
            blocks.push(SemanticDecoderBlock::new(
                in_ch,
                out_ch,
                stride,
                &cfg.block_dilations,
                cfg.unit_kernel_size,
                vb.pp("conv_blocks").pp(i),
            )?);
        }

        let conv2 = candle_nn::conv1d_no_bias(
            sem_hidden,
            sem_hidden,
            k,
            Conv1dConfig {
                padding: k / 2,
                ..Default::default()
            },
            vb.pp("conv2"),
        )?;

        Ok(Self {
            conv1,
            conv_blocks: blocks,
            conv2,
        })
    }
}

impl Module for SemanticDecoder {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mut h = xs.apply(&self.conv1)?;
        for block in &self.conv_blocks {
            h = block.forward(&h)?;
        }
        h.apply(&self.conv2)
    }
}
