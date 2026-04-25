// the_lab/mod.rs — Inference Engine
// "This is not meth. This is art."

pub mod engine;
pub mod gguf;
pub mod prompt;
pub mod server;
pub mod turbo_quant;

pub use engine::InferenceEngine;
pub use prompt::ChatMessage;
