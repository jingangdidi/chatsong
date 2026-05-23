use serde::Deserialize;

/// Generates a named function returning a constant default value for serde.
macro_rules! serde_default {
    ($name:ident, $ty:ty, $val:expr) => {
        fn $name() -> $ty {
            $val
        }
    };
}

// ---------------------------------------------------------------------------
// OmniVoice top-level config (config.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct OmniVoiceConfig {
    #[serde(default = "default_audio_vocab_size")]
    pub audio_vocab_size: usize,
    #[serde(default = "default_audio_mask_id")]
    pub audio_mask_id: usize,
    #[serde(default = "default_num_audio_codebook")]
    pub num_audio_codebook: usize,
    /*
    #[serde(default = "default_audio_codebook_weights")]
    pub audio_codebook_weights: Vec<f64>,
    */
    pub llm_config: Qwen3Config,
}

serde_default!(default_audio_vocab_size, usize, 1025);
serde_default!(default_audio_mask_id, usize, 1024);
serde_default!(default_num_audio_codebook, usize, 8);

/*
fn default_audio_codebook_weights() -> Vec<f64> {
    vec![8.0, 8.0, 6.0, 6.0, 4.0, 4.0, 2.0, 2.0]
}

impl OmniVoiceConfig {
    pub fn normalized_codebook_weights(&self) -> Vec<f64> {
        let sum: f64 = self.audio_codebook_weights.iter().sum();
        self.audio_codebook_weights
            .iter()
            .map(|w| w / sum)
            .collect()
    }
}
*/

// ---------------------------------------------------------------------------
// Qwen3 LLM config (nested under llm_config)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Qwen3Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    #[serde(default = "default_head_dim")]
    pub head_dim: usize,
    #[serde(default)]
    pub attention_bias: bool,
    pub num_key_value_heads: usize,
    #[serde(default = "default_max_position_embeddings")]
    pub max_position_embeddings: usize,
    #[serde(default = "default_rope_theta")]
    pub rope_theta: f64,
    #[serde(default = "default_rms_norm_eps")]
    pub rms_norm_eps: f64,
    /*
    #[serde(default = "default_hidden_act")]
    pub hidden_act: String,
    #[serde(default)]
    pub tie_word_embeddings: bool,
    */
}

serde_default!(default_head_dim, usize, 128);
serde_default!(default_max_position_embeddings, usize, 40960);
serde_default!(default_rope_theta, f64, 1_000_000.0);
serde_default!(default_rms_norm_eps, f64, 1e-6);

/*
fn default_hidden_act() -> String {
    "silu".to_string()
}
*/

// ---------------------------------------------------------------------------
// HiggsAudioV2 tokenizer config (audio_tokenizer/config.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct HiggsAudioV2Config {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: usize,
    #[serde(default = "default_kernel_size")]
    pub kernel_size: usize,
    #[serde(default = "default_channel_ratios")]
    pub channel_ratios: Vec<f64>,
    #[serde(default = "default_strides")]
    pub strides: Vec<usize>,
    #[serde(default = "default_block_dilations")]
    pub block_dilations: Vec<usize>,
    #[serde(default = "default_unit_kernel_size")]
    pub unit_kernel_size: usize,
    #[serde(default = "default_codebook_size")]
    pub codebook_size: usize,
    #[serde(default = "default_codebook_dim")]
    pub codebook_dim: usize,
    #[serde(default = "default_target_bandwidths")]
    pub target_bandwidths: Vec<f64>,
    #[serde(default = "default_semantic_sample_rate")]
    pub semantic_sample_rate: usize,
    #[serde(default = "default_downsample_factor")]
    pub downsample_factor: usize,

    pub acoustic_model_config: Option<DacConfig>,
    pub semantic_model_config: Option<HuBERTConfig>,
}

serde_default!(default_sample_rate, usize, 24000);
serde_default!(default_kernel_size, usize, 3);
fn default_channel_ratios() -> Vec<f64> {
    vec![1.0, 1.0]
}
fn default_strides() -> Vec<usize> {
    vec![1, 1]
}
fn default_block_dilations() -> Vec<usize> {
    vec![1, 1]
}
serde_default!(default_unit_kernel_size, usize, 3);
serde_default!(default_codebook_size, usize, 1024);
serde_default!(default_codebook_dim, usize, 64);
fn default_target_bandwidths() -> Vec<f64> {
    vec![0.5, 1.0, 1.5, 2.0, 4.0]
}
serde_default!(default_semantic_sample_rate, usize, 16000);
serde_default!(default_downsample_factor, usize, 320);

impl HiggsAudioV2Config {
    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    pub fn hop_length(&self) -> usize {
        let dac = self.dac_config();
        dac.downsampling_ratios.iter().product()
    }

    pub fn frame_rate(&self) -> usize {
        (self.sample_rate as f64 / self.hop_length() as f64).ceil() as usize
    }

    pub fn hidden_size(&self) -> usize {
        self.dac_config().hidden_size + self.hubert_config().hidden_size
    }

    pub fn semantic_hidden_size(&self) -> usize {
        self.hubert_config().hidden_size
    }

    pub fn num_quantizers(&self) -> usize {
        let bw = self.target_bandwidths.last().copied().unwrap_or(2.0);
        let nbits = (self.codebook_size as f64).log2().ceil() as usize;
        (1000.0 * bw / (self.frame_rate() * nbits) as f64) as usize
    }

    pub fn semantic_downsample_factor(&self) -> usize {
        let hop = self.hop_length();
        let ratio = self.sample_rate as f64 / self.semantic_sample_rate as f64;
        (hop as f64 / ratio / self.downsample_factor as f64) as usize
    }

    /// Returns the DAC (acoustic) sub-config.
    ///
    /// # Panics
    ///
    /// Panics if `acoustic_model_config` is absent from the JSON. Call
    /// [`try_dac_config`](Self::try_dac_config) for a non-panicking variant.
    pub fn dac_config(&self) -> &DacConfig {
        self.try_dac_config()
            .expect("acoustic_model_config is required in audio_tokenizer/config.json")
    }

    /// Returns the HuBERT (semantic) sub-config.
    ///
    /// # Panics
    ///
    /// Panics if `semantic_model_config` is absent from the JSON. Call
    /// [`try_hubert_config`](Self::try_hubert_config) for a non-panicking variant.
    pub fn hubert_config(&self) -> &HuBERTConfig {
        self.try_hubert_config()
            .expect("semantic_model_config is required in audio_tokenizer/config.json")
    }

    /// Non-panicking accessor for the DAC sub-config.
    pub fn try_dac_config(&self) -> Option<&DacConfig> {
        self.acoustic_model_config.as_ref()
    }

    /// Non-panicking accessor for the HuBERT sub-config.
    pub fn try_hubert_config(&self) -> Option<&HuBERTConfig> {
        self.semantic_model_config.as_ref()
    }
}

// ---------------------------------------------------------------------------
// DAC (Descript Audio Codec) config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct DacConfig {
    #[serde(default = "default_dac_encoder_hidden")]
    pub encoder_hidden_size: usize,
    #[serde(default = "default_dac_decoder_hidden")]
    pub decoder_hidden_size: usize,
    #[serde(default = "default_dac_hidden")]
    pub hidden_size: usize,
    #[serde(default = "default_dac_downsampling")]
    pub downsampling_ratios: Vec<usize>,
    #[serde(default = "default_dac_upsampling")]
    pub upsampling_ratios: Vec<usize>,
    /*
    #[serde(default = "default_dac_n_codebooks")]
    pub n_codebooks: Option<usize>,
    #[serde(default = "default_dac_codebook_size")]
    pub codebook_size: Option<usize>,
    #[serde(default = "default_dac_codebook_dim")]
    pub codebook_dim: Option<usize>,
    */
}

serde_default!(default_dac_encoder_hidden, usize, 64);
serde_default!(default_dac_decoder_hidden, usize, 1024);
serde_default!(default_dac_hidden, usize, 256);
fn default_dac_downsampling() -> Vec<usize> {
    vec![8, 5, 4, 2, 3]
}
fn default_dac_upsampling() -> Vec<usize> {
    vec![8, 5, 4, 2, 3]
}

/*
serde_default!(default_dac_n_codebooks, Option<usize>, Some(9));
serde_default!(default_dac_codebook_size, Option<usize>, Some(1024));
serde_default!(default_dac_codebook_dim, Option<usize>, Some(8));
*/

// ---------------------------------------------------------------------------
// HuBERT config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct HuBERTConfig {
    #[serde(default = "default_hubert_hidden")]
    pub hidden_size: usize,
    #[serde(default = "default_hubert_num_hidden_layers")]
    pub num_hidden_layers: usize,
    #[serde(default = "default_hubert_num_attention_heads")]
    pub num_attention_heads: usize,
    #[serde(default = "default_hubert_intermediate_size")]
    pub intermediate_size: usize,
    /*
    #[serde(default = "default_hubert_hidden_act")]
    pub hidden_act: String,
    */
    #[serde(default = "default_hubert_layer_norm_eps")]
    pub layer_norm_eps: f64,
    #[serde(default = "default_hubert_feat_extract_norm")]
    pub feat_extract_norm: String,
    #[serde(default = "default_hubert_conv_dim")]
    pub conv_dim: Vec<usize>,
    #[serde(default = "default_hubert_conv_stride")]
    pub conv_stride: Vec<usize>,
    #[serde(default = "default_hubert_conv_kernel")]
    pub conv_kernel: Vec<usize>,
    #[serde(default = "default_hubert_num_conv_pos_embeddings")]
    pub num_conv_pos_embeddings: usize,
    #[serde(default = "default_hubert_num_conv_pos_embedding_groups")]
    pub num_conv_pos_embedding_groups: usize,
    #[serde(default = "default_hubert_conv_bias")]
    pub conv_bias: bool,
}

serde_default!(default_hubert_hidden, usize, 768);
serde_default!(default_hubert_num_hidden_layers, usize, 12);
serde_default!(default_hubert_num_attention_heads, usize, 12);
serde_default!(default_hubert_intermediate_size, usize, 3072);

/*
fn default_hubert_hidden_act() -> String {
    "gelu".to_string()
}
*/

serde_default!(default_hubert_layer_norm_eps, f64, 1e-5);
fn default_hubert_feat_extract_norm() -> String {
    "group".to_string()
}
fn default_hubert_conv_dim() -> Vec<usize> {
    vec![512, 512, 512, 512, 512, 512, 512]
}
fn default_hubert_conv_stride() -> Vec<usize> {
    vec![5, 2, 2, 2, 2, 2, 2]
}
fn default_hubert_conv_kernel() -> Vec<usize> {
    vec![10, 3, 3, 3, 3, 2, 2]
}
serde_default!(default_hubert_num_conv_pos_embeddings, usize, 128);
serde_default!(default_hubert_num_conv_pos_embedding_groups, usize, 16);
serde_default!(default_hubert_conv_bias, bool, false);
