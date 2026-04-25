# Heisenberg: Autonomous AI Agent

*"I am the one who knocks."*

Heisenberg is a fully local, autonomous AI agent built for constrained hardware (Linux/macOS). It leverages a dual-runtime architecture: **Rust** for maximum efficiency and memory control, and **Bash** for native system operations.

## Core Features

- **Universal GGUF Support**: Drop any `.gguf` model into `models/` — Heisenberg auto-detects architecture, quantization, and prompt format. Supports Qwen, Llama 2/3, Gemma, Phi, Mistral, DeepSeek, Vicuna, Alpaca, and more.
- **TurboQuant Integration**: Adaptive KV cache quantization that lets you run 9B parameter models on 3 GB RAM. Auto-selects the optimal KV cache compression (Q8_0 → Q4_0) and context window based on available memory.
- **Strict RAM Enforcement (3 GB)**: The `mike` memory manager monitors RSS in real-time, dynamically adjusts context windows, and enforces a hard 3 GB cap.
- **Dual-Runtime**: Rust orchestration (`heisenberg`) mapping structured tasks to Bash shell sandboxes (`jesse`).
- **System Automation**: Screen captures (`hank`), sandboxed browser drivers (`the_rv`), and self-modification (`blue_sky`).
- **Autonomous Task Execution**: The `gus` scheduler decomposes tasks into steps and executes them via the LLM-powered action loop.

## Building and Running

1. Compile the project:
   ```bash
   bash scripts/build.sh
   ```

2. Enter the interactive agent shell:
   ```bash
   bash scripts/say-my-name.sh chat
   ```

3. Run an autonomous task:
   ```bash
   bash scripts/say-my-name.sh run "create a hello world python script"
   ```

4. Check status:
   ```bash
   bash scripts/say-my-name.sh status
   ```

## Supported Models

Place any `.gguf` model in the `models/` directory. Examples:

| Model               | Size   | RAM Usage (TurboQuant) |
| ------------------- | ------ | ---------------------- |
| Qwen2.5-3B-Q4_K_M   | 1.9 GB | ~1.5 GB                |
| Llama-3.1-8B-Q4_K_M | 4.7 GB | ~2.5 GB                |
| Gemma-2-9B-Q3_K_M   | 3.8 GB | ~2.8 GB                |
| Phi-3-Mini-Q4_K_M   | 2.3 GB | ~1.8 GB                |
| Mistral-7B-Q4_K_M   | 4.1 GB | ~2.4 GB                |

## TurboQuant: How It Works

TurboQuant uses adaptive KV cache quantization to dramatically reduce memory:

1. **Model Scanning**: Reads GGUF headers to extract architecture, layer count, head dimensions
2. **Memory Planning**: Calculates KV cache size for each (quantization, context) combination
3. **Adaptive Compression**: Selects Q8_0 → Q5_0 → Q4_0 KV cache based on budget
4. **Context Scaling**: Reduces context window when KV cache alone isn't enough
5. **mmap + Flash Attention**: Always-on for large models to minimize working set

The compression ratio vs FP16 KV cache:
- Q8_0: 2x compression
- Q5_0: 3.2x compression
- Q4_0: 4x compression

## Architecture

- **`src/heisenberg/`** (*Walter*): Core orchestrator, REPL, and action dispatch.
- **`src/the_lab/`**: Inference subsystem — GGUF scanner, TurboQuant planner, llama-server management.
- **`src/mike/`**: Memory manager — RSS monitoring, zone tracking, pressure-aware throttling.
- **`src/mike/db.rs`**: PostgreSQL integration for persistent long-term memory.
- **`src/gus/`**: Task scheduler — queue-based task decomposition and execution tracking.
- **`src/jesse/`**: OS-level Bash executor, output capture, sandboxing.
- **`src/saul/`**: Configuration loader, runtime policy enforcement.
- **`src/hank/`**: Screen observer (screenshots).
- **`src/the_rv/`**: Browser sandbox / automation.
- **`src/blue_sky/`**: Self-modification with test validation and rollback.

## Slash Commands

| Command   | Description                            |
| --------- | -------------------------------------- |
| `/status` | Show RAM, model, and TurboQuant status |
| `/turbo`  | Detailed TurboQuant compression report |
| `/model`  | Show active model info                 |
| `/clear`  | Clear conversation context             |
| `/help`   | Show available commands                |
| `/q`      | Exit session                           |

## License

MIT
# Heisenberg
