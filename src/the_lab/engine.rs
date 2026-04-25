// the_lab/engine.rs -- Inference engine with TurboQuant integration
// "This is not meth. This is art."

use crate::mike::Mike;
use crate::saul::InferenceConfig;
use crate::the_lab::gguf::{self, GgufModelInfo, PromptFormat, TurboQuantPlan};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::prompt::{ChatMessage, PromptBuilder};
use super::server::LlamaServer;

pub struct InferenceEngine {
    config: InferenceConfig,
    server: Option<LlamaServer>,
    client: reqwest::Client,
    /// The active model's prompt format (detected from GGUF)
    prompt_format: PromptFormat,
    /// Active TurboQuant plan
    turbo_plan: Option<TurboQuantPlan>,
    /// Active model info
    model_info: Option<GgufModelInfo>,
}

#[derive(Debug, Serialize)]
struct CompletionRequest {
    prompt: String,
    n_predict: u32,
    temperature: f32,
    stream: bool,
    stop: Vec<String>,
    n_ctx: u32,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    content: String,
    #[allow(dead_code)]
    #[serde(default)]
    stopped_eos: bool,
}

impl InferenceEngine {
    pub fn new(config: InferenceConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            config,
            server: None,
            client,
            prompt_format: PromptFormat::ChatML, // default, will be updated
            turbo_plan: None,
            model_info: None,
        }
    }

    pub async fn start(&mut self, mike: &Mike) -> Result<()> {
        // ── Step 1: Scan models directory for all GGUF files ──
        let models_dir = if self.config.model_path.is_dir() {
            self.config.model_path.clone()
        } else {
            self.config
                .model_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("models"))
                .to_path_buf()
        };

        let (model_info, turbo_plan) = if models_dir.exists() && models_dir.is_dir() {
            match gguf::scan_models(&models_dir) {
                Ok(models) => {
                    info!(
                        "Found {} GGUF model(s). Selecting best for {:.0}MB budget...",
                        models.len(),
                        mike.max_bytes as f64 / 1e6
                    );

                    // Select best model that fits RAM budget
                    gguf::select_best_model(&models, mike.max_bytes)?
                }
                Err(e) => {
                    warn!(
                        "Model scan failed: {}. Falling back to configured model.",
                        e
                    );
                    self.fallback_model_info(mike)?
                }
            }
        } else {
            // Fall back to the configured model path directly
            self.fallback_model_info(mike)?
        };

        // ── Step 2: Store model metadata ──
        self.prompt_format = model_info.prompt_format;
        self.model_info = Some(model_info.clone());
        self.turbo_plan = Some(turbo_plan.clone());

        info!(
            "Model: {} | Format: {:?} | TurboQuant: K={} V={} ctx={} batch={}",
            model_info.display_name(),
            self.prompt_format,
            turbo_plan.kv_cache_k_type,
            turbo_plan.kv_cache_v_type,
            turbo_plan.context_length,
            turbo_plan.batch_size,
        );

        // ── Step 3: Launch llama-server with TurboQuant parameters ──
        let mut server = LlamaServer::new(
            self.config.llama_server_bin.clone(),
            model_info.path.clone(),
            self.config.server_port,
            self.config.threads,
            self.config.gpu_layers,
            turbo_plan,
        );
        server
            .start()
            .await
            .context("Failed to start llama-cpp server")?;
        self.wait_ready().await?;
        self.server = Some(server);

        // ── Step 4: Update zone tracking ──
        mike.zones.set(
            crate::mike::zones::MemoryZone::Inference,
            model_info.file_size_bytes,
        );

        info!(
            "The lab is OPEN. {} ready on :{} with TurboQuant",
            model_info.display_name(),
            self.config.server_port
        );
        Ok(())
    }

    /// Fallback: parse the single configured model file
    fn fallback_model_info(&self, mike: &Mike) -> Result<(GgufModelInfo, TurboQuantPlan)> {
        if self.config.model_path.is_dir() {
            anyhow::bail!("No GGUF models found in directory: {}", self.config.model_path.display());
        }
        let model_info = gguf::parse_gguf_header(&self.config.model_path)?;
        let plan = gguf::plan_turbo_quant(&model_info, mike.max_bytes);
        Ok((model_info, plan))
    }

    pub async fn stop(&mut self) {
        if let Some(mut s) = self.server.take() {
            s.stop().await;
        }
        self.turbo_plan = None;
        self.model_info = None;
    }

    pub async fn generate(&self, messages: &[ChatMessage], mike: &Mike) -> Result<String> {
        let plan = self
            .turbo_plan
            .as_ref()
            .map(|p| p.context_length)
            .unwrap_or(self.config.context_length);

        // Adapt context based on current memory pressure
        let ctx = mike.recommended_context_size(plan);

        let max_tokens = match mike.pressure() {
            crate::mike::MemoryPressure::Critical => self.config.max_tokens / 4,
            crate::mike::MemoryPressure::High => self.config.max_tokens / 2,
            _ => self.config.max_tokens,
        };

        // Build prompt using the model's detected format
        let prompt = PromptBuilder::build(messages, self.prompt_format);
        let stop = self.prompt_format.stop_tokens();

        debug!(
            "Generating: ctx={} max_tokens={} prompt_chars={} format={:?} stops={:?}",
            ctx,
            max_tokens,
            prompt.len(),
            self.prompt_format,
            stop
        );

        let url = format!("http://127.0.0.1:{}/completion", self.config.server_port);
        let req = CompletionRequest {
            prompt,
            n_predict: max_tokens,
            temperature: self.config.temperature,
            stream: false,
            stop,
            n_ctx: ctx,
        };

        let resp = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .context("Failed to call llama-cpp server")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("llama-cpp server error {}: {}", status, body);
        }

        let data: CompletionResponse = resp
            .json()
            .await
            .context("Failed to parse completion response")?;

        // Refresh memory monitor after generation
        mike.monitor.refresh();

        Ok(data.content.trim().to_string())
    }

    /// Poll until the llama-cpp server responds to health checks.
    async fn wait_ready(&self) -> Result<()> {
        let url = format!("http://127.0.0.1:{}/health", self.config.server_port);
        for attempt in 0..180u32 {
            match self.client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    info!("llama-server healthy after {}s", attempt);
                    return Ok(());
                }
                _ => {
                    if attempt % 10 == 0 && attempt > 0 {
                        warn!("Waiting for llama-server... ({}s elapsed)", attempt);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
        anyhow::bail!("llama-server did not become ready within 180s")
    }

    pub fn is_running(&self) -> bool {
        self.server.is_some()
    }

    /// Get the active model's display name
    pub fn model_name(&self) -> String {
        self.model_info
            .as_ref()
            .map(|m| m.display_name())
            .unwrap_or_else(|| "No model".into())
    }

    /// Get the active prompt format
    pub fn prompt_format(&self) -> PromptFormat {
        self.prompt_format
    }

    /// Get the active TurboQuant plan
    pub fn active_plan(&self) -> Option<&TurboQuantPlan> {
        self.turbo_plan.as_ref()
    }
}
