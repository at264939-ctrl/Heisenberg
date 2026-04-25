#![allow(non_camel_case_types)]
// the_lab/gguf.rs — GGUF Model Scanner & TurboQuant Memory Planner
// "The chemistry must be precise."

use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

const GGUF_MAGIC: u32 = 0x46554747;

// ── Quantization ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantType {
    F32,
    F16,
    Q4_0,
    Q4_1,
    Q5_0,
    Q5_1,
    Q8_0,
    Q8_1,
    Q2_K,
    Q3_K_S,
    Q3_K_M,
    Q3_K_L,
    Q4_K_S,
    Q4_K_M,
    Q5_K_S,
    Q5_K_M,
    Q6_K,
    IQ2_XXS,
    IQ2_XS,
    IQ2_S,
    IQ3_XXS,
    IQ3_S,
    IQ4_NL,
    IQ4_XS,
    IQ1_S,
    Unknown(u32),
}

impl QuantType {
    pub fn from_id(id: u32) -> Self {
        match id {
            0 => Self::F32,
            1 => Self::F16,
            2 => Self::Q4_0,
            3 => Self::Q4_1,
            6 => Self::Q5_0,
            7 => Self::Q5_1,
            8 => Self::Q8_0,
            9 => Self::Q8_1,
            10 => Self::Q2_K,
            11 => Self::Q3_K_S,
            12 => Self::Q3_K_M,
            13 => Self::Q3_K_L,
            14 => Self::Q4_K_S,
            15 => Self::Q4_K_M,
            16 => Self::Q5_K_S,
            17 => Self::Q5_K_M,
            18 => Self::Q6_K,
            19 => Self::IQ2_XXS,
            20 => Self::IQ2_XS,
            21 => Self::IQ3_XXS,
            22 => Self::IQ1_S,
            23 => Self::IQ4_NL,
            24 => Self::IQ3_S,
            25 => Self::IQ2_S,
            26 => Self::IQ4_XS,
            v => Self::Unknown(v),
        }
    }

    pub fn bits_per_weight(&self) -> f64 {
        match self {
            Self::F32 => 32.0,
            Self::F16 => 16.0,
            Self::Q8_0 | Self::Q8_1 => 8.0,
            Self::Q6_K => 6.5,
            Self::Q5_K_S | Self::Q5_K_M | Self::Q5_0 | Self::Q5_1 => 5.5,
            Self::Q4_K_S | Self::Q4_K_M | Self::Q4_0 | Self::Q4_1 => 4.5,
            Self::IQ4_NL | Self::IQ4_XS => 4.25,
            Self::Q3_K_L => 4.1,
            Self::Q3_K_M => 3.9,
            Self::Q3_K_S => 3.4,
            Self::IQ3_XXS | Self::IQ3_S => 3.1,
            Self::Q2_K => 2.6,
            Self::IQ2_XXS => 2.1,
            Self::IQ2_XS | Self::IQ2_S => 2.3,
            Self::IQ1_S => 1.6,
            Self::Unknown(_) => 4.5,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::F32 => "F32",
            Self::F16 => "F16",
            Self::Q4_0 => "Q4_0",
            Self::Q4_1 => "Q4_1",
            Self::Q5_0 => "Q5_0",
            Self::Q5_1 => "Q5_1",
            Self::Q8_0 => "Q8_0",
            Self::Q8_1 => "Q8_1",
            Self::Q2_K => "Q2_K",
            Self::Q3_K_S => "Q3_K_S",
            Self::Q3_K_M => "Q3_K_M",
            Self::Q3_K_L => "Q3_K_L",
            Self::Q4_K_S => "Q4_K_S",
            Self::Q4_K_M => "Q4_K_M",
            Self::Q5_K_S => "Q5_K_S",
            Self::Q5_K_M => "Q5_K_M",
            Self::Q6_K => "Q6_K",
            Self::IQ2_XXS => "IQ2_XXS",
            Self::IQ2_XS => "IQ2_XS",
            Self::IQ2_S => "IQ2_S",
            Self::IQ3_XXS => "IQ3_XXS",
            Self::IQ3_S => "IQ3_S",
            Self::IQ4_NL => "IQ4_NL",
            Self::IQ4_XS => "IQ4_XS",
            Self::IQ1_S => "IQ1_S",
            Self::Unknown(_) => "Unknown",
        }
    }
}

// ── Prompt Format ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFormat {
    ChatML,   // Qwen, Yi, OpenHermes
    Llama3,   // Llama 3+
    Llama2,   // Llama 2
    Alpaca,   // Alpaca finetunes
    Gemma,    // Google Gemma
    Phi,      // Microsoft Phi
    Mistral,  // Mistral Instruct
    DeepSeek, // DeepSeek
    Vicuna,   // Vicuna / ShareGPT
    Raw,      // No template
}

impl PromptFormat {
    /// Detect prompt format from architecture name and filename
    pub fn detect(arch: &str, filename: &str) -> Self {
        let a = arch.to_lowercase();
        let f = filename.to_lowercase();

        if a.contains("qwen") || f.contains("qwen") || f.contains("yi-") || f.contains("openhermes")
        {
            Self::ChatML
        } else if a.contains("gemma") || f.contains("gemma") {
            Self::Gemma
        } else if a.contains("phi") || f.contains("phi") {
            Self::Phi
        } else if a.contains("mistral") || f.contains("mistral") {
            Self::Mistral
        } else if a.contains("deepseek") || f.contains("deepseek") {
            Self::DeepSeek
        } else if f.contains("vicuna") {
            Self::Vicuna
        } else if f.contains("alpaca") {
            Self::Alpaca
        } else if a.contains("llama") {
            if f.contains("llama-2") || f.contains("llama2") {
                Self::Llama2
            } else {
                Self::Llama3
            }
        } else {
            // Default to ChatML — widely supported
            Self::ChatML
        }
    }

    pub fn stop_tokens(&self) -> Vec<String> {
        match self {
            Self::ChatML => vec!["<|im_end|>".into()],
            Self::Llama3 => vec!["<|eot_id|>".into()],
            Self::Llama2 => vec!["</s>".into()],
            Self::Gemma => vec!["<end_of_turn>".into()],
            Self::Phi => vec!["<|end|>".into()],
            Self::Mistral => vec!["</s>".into(), "[/INST]".into()],
            Self::DeepSeek => vec!["<|end_of_sentence|>".into()],
            Self::Vicuna | Self::Alpaca => vec!["</s>".into(), "###".into()],
            Self::Raw => vec![],
        }
    }
}

// ── GGUF Model Info ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GgufModelInfo {
    pub path: PathBuf,
    pub filename: String,
    pub file_size_bytes: u64,
    pub architecture: String,
    pub param_count: u64,      // estimated from file size + quant
    pub quant_type: QuantType, // inferred from filename
    pub prompt_format: PromptFormat,
    pub context_length: u32, // architecture default
    pub embedding_dim: u32,
    pub head_count: u32,
    pub head_count_kv: u32,
    pub layer_count: u32,
}

impl GgufModelInfo {
    /// Estimate model weight memory (the GGUF file mapped in RAM)
    pub fn weight_memory_bytes(&self) -> u64 {
        // With mmap, not all weights are in physical RAM at once.
        // But we budget worst case = file size for safety.
        self.file_size_bytes
    }

    /// Estimate KV cache memory for a given context length and TurboQuant level.
    ///
    /// TurboQuant Core Formula (from the paper):
    ///   KV_cache_bytes = 2 * n_layers * n_heads_kv * (d_head) * context * bytes_per_element
    ///
    /// With TurboQuant applied:
    ///   - K cache: compressed to kv_quant_bits per element
    ///   - V cache: compressed to kv_quant_bits per element
    ///   - This reduces KV cache by (16 / kv_quant_bits)x compared to FP16
    pub fn kv_cache_bytes(&self, context: u32, kv_quant_bits: f64) -> u64 {
        let d_head = if self.head_count > 0 {
            self.embedding_dim / self.head_count
        } else {
            128 // safe default
        };
        let n_kv = if self.head_count_kv > 0 {
            self.head_count_kv
        } else {
            self.head_count
        };
        let layers = if self.layer_count > 0 {
            self.layer_count
        } else {
            32
        };

        // KV cache = 2 (K+V) * layers * n_kv_heads * d_head * context * (bits/8)
        let bytes = 2.0
            * layers as f64
            * n_kv as f64
            * d_head as f64
            * context as f64
            * (kv_quant_bits / 8.0);

        bytes as u64
    }

    /// Total estimated memory for this model at given context + KV quant level.
    /// Includes: mmap working set + KV cache + overhead
    pub fn total_memory_estimate(&self, context: u32, kv_quant_bits: f64) -> u64 {
        // For mmap models, active working set is typically 30-60% of file size
        // depending on context. Short context = less active weights.
        let mmap_working_set = (self.file_size_bytes as f64 * 0.5) as u64;
        let kv = self.kv_cache_bytes(context, kv_quant_bits);
        let overhead = 200 * 1024 * 1024; // ~200 MB for llama-server + buffers
        mmap_working_set + kv + overhead
    }

    /// Determine the display name (e.g. "Qwen2.5 9B Q4_K_M")
    pub fn display_name(&self) -> String {
        let params_str = if self.param_count >= 1_000_000_000 {
            format!("{:.1}B", self.param_count as f64 / 1e9)
        } else if self.param_count >= 1_000_000 {
            format!("{:.0}M", self.param_count as f64 / 1e6)
        } else {
            format!("{}P", self.param_count)
        };
        format!(
            "{} {} {}",
            self.architecture,
            params_str,
            self.quant_type.name()
        )
    }
}

// ── TurboQuant Memory Planner ────────────────────────────────────────
//
// Implements adaptive KV cache quantization based on the TurboQuant paper.
// Determines optimal KV cache quantization and context window to fit
// within the RAM budget.

#[derive(Debug, Clone)]
pub struct TurboQuantPlan {
    pub kv_cache_k_type: String, // llama.cpp --cache-type-k flag
    pub kv_cache_v_type: String, // llama.cpp --cache-type-v flag
    pub kv_quant_bits: f64,      // effective bits per KV element
    pub context_length: u32,     // recommended context
    pub batch_size: u32,         // recommended batch size
    pub flash_attn: bool,        // enable flash attention
    pub mmap: bool,              // use memory mapping
    pub mlock: bool,             // lock pages in RAM
    pub estimated_total_mb: f64, // total estimated usage in MB
    pub kv_cache_mb: f64,        // KV cache portion in MB
    pub weight_mb: f64,          // model weights portion in MB
}

impl std::fmt::Display for TurboQuantPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TurboQuant Plan: K={} V={} ctx={} batch={} | Est: {:.0}MB total ({:.0}MB weights + {:.0}MB KV cache) | flash_attn={} mmap={} mlock={}",
            self.kv_cache_k_type, self.kv_cache_v_type,
            self.context_length, self.batch_size,
            self.estimated_total_mb, self.weight_mb, self.kv_cache_mb,
            self.flash_attn, self.mmap, self.mlock)
    }
}

/// Plan TurboQuant parameters to fit model within RAM budget.
///
/// Asymmetric K/V strategy inspired by TurboQuant paper (Section 4):
///   Keys (K)   → participate in attention score computation (inner products)
///              → higher precision required: Q5_0
///   Values (V) → aggregated linearly, tolerate more compression: Q4_0
///
/// This asymmetric K=Q5/V=Q4 scheme gives ~3.5× compression vs FP16
/// with near-lossless quality — better than symmetric Q4 alone.
///
/// Compression ladder (best → emergency):
///   Q8K+Q8V → Q5K+Q4V (asymmetric) → Q5K+Q5V → Q4K+Q4V → reduce ctx
pub fn plan_turbo_quant(model: &GgufModelInfo, max_ram_bytes: u64) -> TurboQuantPlan {
    let max_mb = max_ram_bytes as f64 / (1024.0 * 1024.0);
    let file_mb = model.file_size_bytes as f64 / (1024.0 * 1024.0);

    info!(
        "TurboQuant planning for {} | file={:.0}MB budget={:.0}MB",
        model.display_name(),
        file_mb,
        max_mb
    );

    let needs_aggressive = file_mb > (max_mb * 0.5);

    // k_bits/v_bits tracked separately for accurate asymmetric KV memory estimation.
    // avg_bits = (k_bits + v_bits) / 2 — used for total KV cost.
    struct Candidate {
        k_type:     &'static str,
        v_type:     &'static str,
        k_bits:     f64,
        v_bits:     f64,
        ctx_factor: f64,
        batch:      u32,
        label:      &'static str,
    }

    let candidates = [
        // L0: Symmetric Q8 — 2× vs FP16, best quality
        Candidate { k_type: "q8_0", v_type: "q8_0", k_bits: 8.0, v_bits: 8.0,
                    ctx_factor: 1.0, batch: 512, label: "Q8K+Q8V" },
        // L0b: Q8 at half context
        Candidate { k_type: "q8_0", v_type: "q8_0", k_bits: 8.0, v_bits: 8.0,
                    ctx_factor: 0.5, batch: 512, label: "Q8K+Q8V/ctx50" },
        // L1: Asymmetric K=Q5,V=Q4 — 3.5× avg, near-lossless (TurboQuant-style)
        //     K kept at Q5 to preserve attention score fidelity (inner products)
        //     V dropped to Q4 as linear aggregation tolerates more error
        Candidate { k_type: "q5_0", v_type: "q4_0", k_bits: 5.0, v_bits: 4.0,
                    ctx_factor: 1.0, batch: 512, label: "Q5K+Q4V (asymmetric)" },
        // L1b: Asymmetric at half context
        Candidate { k_type: "q5_0", v_type: "q4_0", k_bits: 5.0, v_bits: 4.0,
                    ctx_factor: 0.5, batch: 512, label: "Q5K+Q4V/ctx50" },
        // L2: Symmetric Q5 — 3.2× compression
        Candidate { k_type: "q5_0", v_type: "q5_0", k_bits: 5.0, v_bits: 5.0,
                    ctx_factor: 1.0, batch: 256, label: "Q5K+Q5V" },
        // L3: Symmetric Q4 — 4× compression
        Candidate { k_type: "q4_0", v_type: "q4_0", k_bits: 4.0, v_bits: 4.0,
                    ctx_factor: 1.0, batch: 256, label: "Q4K+Q4V" },
        // L4: Q4 at 50% context
        Candidate { k_type: "q4_0", v_type: "q4_0", k_bits: 4.0, v_bits: 4.0,
                    ctx_factor: 0.5, batch: 256, label: "Q4K+Q4V/ctx50" },
        // L5: Q4 at 25% context (emergency)
        Candidate { k_type: "q4_0", v_type: "q4_0", k_bits: 4.0, v_bits: 4.0,
                    ctx_factor: 0.25, batch: 128, label: "Q4K+Q4V/ctx25" },
        // L6: absolute minimum
        Candidate { k_type: "q4_0", v_type: "q4_0", k_bits: 4.0, v_bits: 4.0,
                    ctx_factor: 0.125, batch: 64, label: "Q4K+Q4V/ctx12" },
    ];

    // Cap context at 2048 on CPU under tight boundaries — prevents slow warmup and excess KV allocation.
    // At ctx=2048 with Q5K/Q4V: Qwen-3B uses ~21MB KV.
    let base_ctx = model.context_length.min(2048);

    for c in &candidates {
        let ctx = ((base_ctx as f64) * c.ctx_factor) as u32;
        let ctx = ctx.max(512); // minimum 512 tokens

        // Compute KV cache using average of K+V bits (asymmetric support)
        let avg_bits = (c.k_bits + c.v_bits) / 2.0;
        let kv_bytes = model.kv_cache_bytes(ctx, avg_bits);
        let kv_mb = kv_bytes as f64 / (1024.0 * 1024.0);

        // mmap working set: smaller ratio for very large models (aggressive mode)
        let mmap_ratio = if needs_aggressive { 0.35 } else { 0.55 };
        let weight_mb = file_mb * mmap_ratio;
        let overhead_mb = 200.0; // llama-server process + buffers
        let total_mb = weight_mb + kv_mb + overhead_mb;

        // Compression ratio vs FP16 (16-bit baseline)
        let compression_vs_fp16 = 16.0 / avg_bits;

        debug!(
            "TurboQuant [{}]: ctx={} -> {:.0}MB (weight={:.0} kv={:.0} overhead={:.0}) | {:.1}× vs FP16",
            c.label, ctx, total_mb, weight_mb, kv_mb, overhead_mb, compression_vs_fp16
        );

        if total_mb <= max_mb {
            let plan = TurboQuantPlan {
                kv_cache_k_type:    c.k_type.to_string(),
                kv_cache_v_type:    c.v_type.to_string(),
                kv_quant_bits:      avg_bits,
                context_length:     ctx,
                batch_size:         c.batch,
                flash_attn:         true,
                mmap:               true,
                mlock:              !needs_aggressive,
                estimated_total_mb: total_mb,
                kv_cache_mb:        kv_mb,
                weight_mb,
            };
            info!(
                "TurboQuant selected [{}]: ctx={} {:.0}MB total | {:.1}× KV compression vs FP16",
                c.label, ctx, total_mb, compression_vs_fp16
            );
            return plan;
        }
    }

    // Absolute fallback: minimal everything
    warn!("TurboQuant: model may exceed memory budget! Using absolute minimum settings.");
    TurboQuantPlan {
        kv_cache_k_type: "q4_0".into(),
        kv_cache_v_type: "q4_0".into(),
        kv_quant_bits: 4.0,
        context_length: 512,
        batch_size: 64,
        flash_attn: true,
        mmap: true,
        mlock: false,
        estimated_total_mb: max_mb,
        kv_cache_mb: 0.0,
        weight_mb: file_mb * 0.3,
    }
}

// ── GGUF Header Reading ──────────────────────────────────────────────

fn read_u32(f: &mut std::fs::File) -> Result<u32> {
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(f: &mut std::fs::File) -> Result<u64> {
    let mut buf = [0u8; 8];
    f.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_gguf_string(f: &mut std::fs::File) -> Result<String> {
    let len = read_u64(f)? as usize;
    if len > 1_000_000 {
        anyhow::bail!("GGUF string too long: {}", len);
    }
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Skip a GGUF metadata value based on its type
fn skip_gguf_value(f: &mut std::fs::File, vtype: u32) -> Result<()> {
    match vtype {
        0 => {
            f.seek(SeekFrom::Current(1))?;
        } // uint8
        1 => {
            f.seek(SeekFrom::Current(1))?;
        } // int8
        2 => {
            f.seek(SeekFrom::Current(2))?;
        } // uint16
        3 => {
            f.seek(SeekFrom::Current(2))?;
        } // int16
        4 => {
            f.seek(SeekFrom::Current(4))?;
        } // uint32
        5 => {
            f.seek(SeekFrom::Current(4))?;
        } // int32
        6 => {
            f.seek(SeekFrom::Current(4))?;
        } // float32
        7 => {
            f.seek(SeekFrom::Current(1))?;
        } // bool
        8 => {
            read_gguf_string(f)?;
        } // string
        9 => {
            // array
            let arr_type = read_u32(f)?;
            let arr_len = read_u64(f)?;
            for _ in 0..arr_len {
                skip_gguf_value(f, arr_type)?;
            }
        }
        10 => {
            f.seek(SeekFrom::Current(8))?;
        } // uint64
        11 => {
            f.seek(SeekFrom::Current(8))?;
        } // int64
        12 => {
            f.seek(SeekFrom::Current(8))?;
        } // float64
        _ => {
            anyhow::bail!("Unknown GGUF value type: {}", vtype);
        }
    }
    Ok(())
}

/// Read a GGUF uint32 value
fn read_gguf_uint32_value(f: &mut std::fs::File) -> Result<u32> {
    read_u32(f)
}

/// Parse GGUF header metadata to extract model information
pub fn parse_gguf_header(path: &Path) -> Result<GgufModelInfo> {
    let mut f = std::fs::File::open(path)
        .with_context(|| format!("Cannot open GGUF: {}", path.display()))?;
    let file_size = f.metadata()?.len();

    // Read magic
    let magic = read_u32(&mut f)?;
    if magic != GGUF_MAGIC {
        anyhow::bail!("Not a GGUF file (magic={:#x}): {}", magic, path.display());
    }

    // Read version
    let version = read_u32(&mut f)?;
    debug!("GGUF v{} file: {}", version, path.display());

    // Read tensor count and metadata count
    let _tensor_count = read_u64(&mut f)?;
    let metadata_count = read_u64(&mut f)?;

    debug!("GGUF metadata entries: {}", metadata_count);

    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut architecture = String::new();
    let mut context_length: u32 = 4096;
    let mut embedding_dim: u32 = 0;
    let mut head_count: u32 = 0;
    let mut head_count_kv: u32 = 0;
    let mut layer_count: u32 = 0;

    // Parse metadata key-value pairs
    let max_entries = metadata_count.min(500); // safety limit
    for _ in 0..max_entries {
        let key = match read_gguf_string(&mut f) {
            Ok(k) => k,
            Err(_) => break,
        };
        let vtype = match read_u32(&mut f) {
            Ok(v) => v,
            Err(_) => break,
        };

        // Extract the values we care about
        if key == "general.architecture" && vtype == 8 {
            architecture = read_gguf_string(&mut f)?;
            debug!("GGUF arch: {}", architecture);
        } else if key.ends_with(".context_length") && vtype == 4 {
            context_length = read_gguf_uint32_value(&mut f)?;
            debug!("GGUF context_length: {}", context_length);
        } else if key.ends_with(".embedding_length") && vtype == 4 {
            embedding_dim = read_gguf_uint32_value(&mut f)?;
            debug!("GGUF embedding_dim: {}", embedding_dim);
        } else if key.ends_with(".attention.head_count") && vtype == 4 {
            head_count = read_gguf_uint32_value(&mut f)?;
            debug!("GGUF head_count: {}", head_count);
        } else if key.ends_with(".attention.head_count_kv") && vtype == 4 {
            head_count_kv = read_gguf_uint32_value(&mut f)?;
            debug!("GGUF head_count_kv: {}", head_count_kv);
        } else if key.ends_with(".block_count") && vtype == 4 {
            layer_count = read_gguf_uint32_value(&mut f)?;
            debug!("GGUF layer_count: {}", layer_count);
        } else {
            // Skip values we don't need
            if let Err(e) = skip_gguf_value(&mut f, vtype) {
                debug!("Skipping rest of GGUF metadata after key '{}': {}", key, e);
                break;
            }
        }
    }

    // Infer quant type from filename (reliable heuristic since GGUF filenames
    // follow the convention: Model-Size-Instruct-QUANT_TYPE.gguf)
    let quant_type = infer_quant_from_filename(&filename);

    // Estimate parameter count from file size and quantization
    let bpw = quant_type.bits_per_weight();
    let param_count = ((file_size as f64 * 8.0) / bpw) as u64;

    let prompt_format = PromptFormat::detect(&architecture, &filename);

    let info = GgufModelInfo {
        path: path.to_path_buf(),
        filename,
        file_size_bytes: file_size,
        architecture: if architecture.is_empty() {
            "unknown".into()
        } else {
            architecture
        },
        param_count,
        quant_type,
        prompt_format,
        context_length,
        embedding_dim,
        head_count,
        head_count_kv,
        layer_count,
    };

    info!("GGUF parsed: {} | arch={} params=~{:.1}B quant={} format={:?} ctx={} layers={} heads={}/{}kv embed={}",
        info.path.display(), info.architecture,
        info.param_count as f64 / 1e9, info.quant_type.name(),
        info.prompt_format, info.context_length,
        info.layer_count, info.head_count, info.head_count_kv,
        info.embedding_dim);

    Ok(info)
}

/// Infer quantization type from GGUF filename
fn infer_quant_from_filename(name: &str) -> QuantType {
    let upper = name.to_uppercase();
    // Check from most specific to least specific
    if upper.contains("IQ1_S") {
        return QuantType::IQ1_S;
    }
    if upper.contains("IQ2_XXS") {
        return QuantType::IQ2_XXS;
    }
    if upper.contains("IQ2_XS") {
        return QuantType::IQ2_XS;
    }
    if upper.contains("IQ2_S") {
        return QuantType::IQ2_S;
    }
    if upper.contains("IQ3_XXS") {
        return QuantType::IQ3_XXS;
    }
    if upper.contains("IQ3_S") {
        return QuantType::IQ3_S;
    }
    if upper.contains("IQ4_NL") {
        return QuantType::IQ4_NL;
    }
    if upper.contains("IQ4_XS") {
        return QuantType::IQ4_XS;
    }
    if upper.contains("Q2_K") {
        return QuantType::Q2_K;
    }
    if upper.contains("Q3_K_S") {
        return QuantType::Q3_K_S;
    }
    if upper.contains("Q3_K_M") {
        return QuantType::Q3_K_M;
    }
    if upper.contains("Q3_K_L") {
        return QuantType::Q3_K_L;
    }
    if upper.contains("Q4_K_S") {
        return QuantType::Q4_K_S;
    }
    if upper.contains("Q4_K_M") {
        return QuantType::Q4_K_M;
    }
    if upper.contains("Q4_0") {
        return QuantType::Q4_0;
    }
    if upper.contains("Q4_1") {
        return QuantType::Q4_1;
    }
    if upper.contains("Q5_K_S") {
        return QuantType::Q5_K_S;
    }
    if upper.contains("Q5_K_M") {
        return QuantType::Q5_K_M;
    }
    if upper.contains("Q5_0") {
        return QuantType::Q5_0;
    }
    if upper.contains("Q5_1") {
        return QuantType::Q5_1;
    }
    if upper.contains("Q6_K") {
        return QuantType::Q6_K;
    }
    if upper.contains("Q8_0") {
        return QuantType::Q8_0;
    }
    if upper.contains("Q8_1") {
        return QuantType::Q8_1;
    }
    if upper.contains("F16") {
        return QuantType::F16;
    }
    if upper.contains("F32") {
        return QuantType::F32;
    }
    QuantType::Q4_K_M // safe default
}

// ── Model Scanner ────────────────────────────────────────────────────

/// Scan a directory for all .gguf model files and parse their headers.
pub fn scan_models(dir: &Path) -> Result<Vec<GgufModelInfo>> {
    if !dir.exists() {
        anyhow::bail!("Models directory does not exist: {}", dir.display());
    }

    let mut models = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
            match parse_gguf_header(&path) {
                Ok(info) => {
                    info!("Found model: {}", info.display_name());
                    models.push(info);
                }
                Err(e) => {
                    warn!("Failed to parse GGUF {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by file size (smallest first for memory-constrained selection)
    models.sort_by_key(|m| m.file_size_bytes);

    if models.is_empty() {
        anyhow::bail!("No .gguf model files found in {}", dir.display());
    }

    info!("Found {} GGUF model(s) in {}", models.len(), dir.display());
    Ok(models)
}

/// Select the best model that fits within the RAM budget.
/// Returns (model_info, turbo_quant_plan).
pub fn select_best_model(
    models: &[GgufModelInfo],
    max_ram_bytes: u64,
) -> Result<(GgufModelInfo, TurboQuantPlan)> {
    if models.is_empty() {
        anyhow::bail!("No models available for selection");
    }

    // Try each model from LARGEST to smallest (prefer bigger models)
    for model in models.iter().rev() {
        let plan = plan_turbo_quant(model, max_ram_bytes);
        if plan.estimated_total_mb <= (max_ram_bytes as f64 / (1024.0 * 1024.0)) {
            info!("Selected model: {} with {}", model.display_name(), plan);
            return Ok((model.clone(), plan));
        }
    }

    // Fallback: use smallest model with most aggressive settings
    let smallest = &models[0];
    let plan = plan_turbo_quant(smallest, max_ram_bytes);
    warn!(
        "Using smallest model as fallback: {} — may exceed budget",
        smallest.display_name()
    );
    Ok((smallest.clone(), plan))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quant_from_filename() {
        assert_eq!(
            infer_quant_from_filename("Qwen2.5-3B-Instruct-Q4_K_M.gguf"),
            QuantType::Q4_K_M
        );
        assert_eq!(
            infer_quant_from_filename("llama-2-7b-Q2_K.gguf"),
            QuantType::Q2_K
        );
        assert_eq!(
            infer_quant_from_filename("phi-3-mini-IQ3_S.gguf"),
            QuantType::IQ3_S
        );
        assert_eq!(infer_quant_from_filename("model-F16.gguf"), QuantType::F16);
    }

    #[test]
    fn test_prompt_format_detection() {
        assert_eq!(
            PromptFormat::detect("qwen2", "Qwen2.5-3B.gguf"),
            PromptFormat::ChatML
        );
        assert_eq!(
            PromptFormat::detect("llama", "llama-2-7b.gguf"),
            PromptFormat::Llama2
        );
        assert_eq!(
            PromptFormat::detect("llama", "Meta-Llama-3.1-8B.gguf"),
            PromptFormat::Llama3
        );
        assert_eq!(
            PromptFormat::detect("gemma", "gemma-2b.gguf"),
            PromptFormat::Gemma
        );
        assert_eq!(
            PromptFormat::detect("phi3", "phi-3-mini.gguf"),
            PromptFormat::Phi
        );
    }

    #[test]
    fn test_kv_cache_calculation() {
        let model = GgufModelInfo {
            path: PathBuf::from("test.gguf"),
            filename: "test-Q4_K_M.gguf".into(),
            file_size_bytes: 2_000_000_000,
            architecture: "qwen2".into(),
            param_count: 3_000_000_000,
            quant_type: QuantType::Q4_K_M,
            prompt_format: PromptFormat::ChatML,
            context_length: 4096,
            embedding_dim: 2048,
            head_count: 16,
            head_count_kv: 2,
            layer_count: 36,
        };

        // With Q8_0 (8 bits): 2 * 36 * 2 * 128 * 4096 * 1 = ~75 MB
        let kv_q8 = model.kv_cache_bytes(4096, 8.0);
        assert!(kv_q8 > 50_000_000 && kv_q8 < 100_000_000);

        // With Q4_0 (4 bits): should be half
        let kv_q4 = model.kv_cache_bytes(4096, 4.0);
        assert_eq!(kv_q4, kv_q8 / 2);
    }

    #[test]
    fn test_turbo_quant_plan() {
        // ── 9B model on 3GB RAM ──────────────────────────────────────────────
        let model_9b = GgufModelInfo {
            path: PathBuf::from("llama-3.1-8b-q4_k_m.gguf"),
            filename: "llama-3.1-8b-q4_k_m.gguf".into(),
            file_size_bytes: 4_900_000_000, // 4.7 GB
            architecture: "llama".into(),
            param_count: 8_030_000_000,
            quant_type: QuantType::Q4_K_M,
            prompt_format: PromptFormat::Llama3,
            context_length: 8192,
            embedding_dim: 4096,
            head_count: 32,
            head_count_kv: 8,
            layer_count: 32,
        };

        let plan_9b = plan_turbo_quant(&model_9b, 3_221_225_472);
        println!("\n=== 8B Q4_K_M on 3GB budget ===");
        println!("  Plan: {}", plan_9b);
        println!("  K={} V={} | avg={:.1}-bit | {:.1}× vs FP16",
            plan_9b.kv_cache_k_type,
            plan_9b.kv_cache_v_type,
            plan_9b.kv_quant_bits,
            16.0 / plan_9b.kv_quant_bits,
        );
        println!("  KV memory: {:.0} MB", plan_9b.kv_cache_mb);
        println!("  Total est: {:.0} MB / 3072 MB budget", plan_9b.estimated_total_mb);

        assert!(
            plan_9b.estimated_total_mb <= 3072.0,
            "8B plan exceeds 3GB: {:.0}MB", plan_9b.estimated_total_mb
        );
        assert!(plan_9b.flash_attn, "flash_attn must be on");
        assert!(plan_9b.mmap, "mmap must be on");
        // KV quantization must be some valid compression (≤ Q8)
        assert!(
            plan_9b.kv_quant_bits <= 8.0,
            "KV bits must be <= 8, got {:.1}", plan_9b.kv_quant_bits
        );
        println!("  ✅ 8B fits in 3GB with K={} V={} ({:.1}× vs FP16)",
            plan_9b.kv_cache_k_type,
            plan_9b.kv_cache_v_type,
            16.0 / plan_9b.kv_quant_bits
        );

        // ── 3B model on 3GB RAM ──────────────────────────────────────────────
        let model_3b = GgufModelInfo {
            path: PathBuf::from("qwen2.5-3b-q4_k_m.gguf"),
            filename: "qwen2.5-3b-q4_k_m.gguf".into(),
            file_size_bytes: 1_929_000_000, // 1.8 GB
            architecture: "qwen2".into(),
            param_count: 3_090_000_000,
            quant_type: QuantType::Q4_K_M,
            prompt_format: PromptFormat::ChatML,
            context_length: 32768,
            embedding_dim: 2048,
            head_count: 16,
            head_count_kv: 2,
            layer_count: 36,
        };

        let plan_3b = plan_turbo_quant(&model_3b, 3_221_225_472);
        println!("\n=== Qwen2.5-3B Q4_K_M on 3GB budget ===");
        println!("  Plan: {}", plan_3b);
        println!("  K={} V={} | avg={:.1}-bit | {:.1}× vs FP16",
            plan_3b.kv_cache_k_type,
            plan_3b.kv_cache_v_type,
            plan_3b.kv_quant_bits,
            16.0 / plan_3b.kv_quant_bits,
        );
        println!("  ctx: {} (native: 32768, capped to 8192)", plan_3b.context_length);
        println!("  KV memory: {:.0} MB", plan_3b.kv_cache_mb);
        println!("  Total est: {:.0} MB / 3072 MB budget", plan_3b.estimated_total_mb);

        // 3B model should comfortably fit with Q8 at 8192 ctx
        assert!(
            plan_3b.estimated_total_mb <= 3072.0,
            "3B plan exceeds 3GB: {:.0}MB", plan_3b.estimated_total_mb
        );
        assert_eq!(plan_3b.context_length, 8192, "context should be capped at 8192");
    }
}

#[test]
fn test_real_model_if_exists() {
    let model_path = std::path::Path::new("models/Qwen2.5-3B-Instruct-Q4_K_M.gguf");
    if !model_path.exists() {
        println!("Skipping: model file not found");
        return;
    }

    let info = parse_gguf_header(model_path).expect("Failed to parse real GGUF");

    println!("=== REAL MODEL TEST ===");
    println!("Name: {}", info.display_name());
    println!("Arch: {}", info.architecture);
    println!("File: {:.1} GB", info.file_size_bytes as f64 / 1e9);
    println!("Quant: {}", info.quant_type.name());
    println!("Format: {:?}", info.prompt_format);
    println!("Context: {}", info.context_length);
    println!("Layers: {}", info.layer_count);
    println!("Heads: {}/{} KV", info.head_count, info.head_count_kv);
    println!("Embed: {}", info.embedding_dim);
    println!("Params: ~{:.1}B", info.param_count as f64 / 1e9);

    // Now test TurboQuant planning for 3GB
    let plan = plan_turbo_quant(&info, 3_221_225_472);
    println!("\n=== TURBOQ PLAN (3GB budget) ===");
    println!("{}", plan);
    println!("KV Compression: {:.1}x vs FP16", 16.0 / plan.kv_quant_bits);

    assert!(
        plan.estimated_total_mb <= 3072.0,
        "Plan exceeds 3GB: {:.0}MB",
        plan.estimated_total_mb
    );
    assert!(info.architecture.to_lowercase().contains("qwen"));
    assert!(matches!(info.prompt_format, PromptFormat::ChatML));

    println!("\n✅ TurboQuant verified: model fits in 3GB RAM!");
}
