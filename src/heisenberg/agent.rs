// heisenberg/agent.rs — Core orchestrator & premium Claude Code-inspired REPL
// "I am the danger."

use crate::blue_sky::patcher::SelfPatcher;
use crate::gus::scheduler::GusScheduler;
use crate::gus::task::Task;
use crate::hank::vision::ScreenObserver;
use crate::jesse::JesseRunner;
use crate::mike::Mike;
use crate::saul::policy::PolicyEngine;
use crate::saul::{Cli, Commands, Config};
use crate::the_lab::{ChatMessage, InferenceEngine};
use crate::the_rv::driver::BrowserDriver;
use anyhow::Result;
use std::io::Write;
use std::sync::Arc;
use tracing::{error, info};

pub struct Orchestrator {
    #[allow(dead_code)]
    config: Config,
    engine: InferenceEngine,
    runner: JesseRunner,
    mike: Arc<Mike>,
    patcher: SelfPatcher,
    browser: BrowserDriver,
    vision: ScreenObserver,
    scheduler: GusScheduler,
    db: Option<crate::mike::DbLedger>,
}

// ── ANSI style palette ───────────────────────────────────────────────
const B: &str = "\x1b[1m";
const D: &str = "\x1b[2m";
const R: &str = "\x1b[0m";
const I: &str = "\x1b[3m";
const GRN: &str = "\x1b[38;5;46m";
const CYN: &str = "\x1b[38;5;87m";
const YLW: &str = "\x1b[38;5;226m";
const ORG: &str = "\x1b[38;5;208m";
const RED: &str = "\x1b[38;5;196m";
const WHT: &str = "\x1b[38;5;255m";
const MAG: &str = "\x1b[38;5;213m";

const BLUE: &str = "\x1b[38;5;75m";

// ── Box-drawing constants ────────────────────────────────────────────
const BOX_TL: &str = "╭";
const BOX_TR: &str = "╮";
const BOX_BL: &str = "╰";
const BOX_BR: &str = "╯";
const BOX_H:  &str = "─";
const BOX_V:  &str = "│";
const BOX_T:  &str = "├";
const BOX_TE: &str = "┤";

impl Orchestrator {
    pub async fn new(config: Config) -> Result<Self> {
        let mike = Arc::new(Mike::new(&config));
        let runner = JesseRunner::new(&config);
        let patcher = SelfPatcher::new(
            PolicyEngine::from_config(&config),
            JesseRunner::new(&config),
        );
        let browser = BrowserDriver::new(
            PolicyEngine::from_config(&config),
            JesseRunner::new(&config),
        );
        let vision = ScreenObserver::new(config.execution.work_dir.clone());
        let engine = InferenceEngine::new(config.inference.clone());
        let scheduler = GusScheduler::new(64);
        let db = crate::mike::DbLedger::connect().await.ok();

        // Start background memory monitoring
        Mike::start_background_monitor(&mike);

        Ok(Self {
            config,
            engine,
            runner,
            mike,
            patcher,
            browser,
            vision,
            scheduler,
            db,
        })
    }

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        match cli.command {
            Some(Commands::Status) => self.cmd_status(),
            Some(Commands::Exec { script, inline }) => self.cmd_exec(script, inline).await?,
            Some(Commands::Chat { prompt }) => self.cmd_chat(prompt).await?,
            Some(Commands::Run { task, dry_run }) => self.cmd_run(task, dry_run).await?,
            None => {
                info!("No command. Showing welcome.");
                self.print_welcome();
            }
            _ => {
                eprintln!("  {YLW}Not yet implemented.{R}");
            }
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════
    //  PREMIUM WELCOME SCREEN (Claude Code-inspired)
    // ══════════════════════════════════════════════════════════════════
    fn print_welcome(&self) {
        let ver = env!("CARGO_PKG_VERSION");
        let cwd = std::env::current_dir()
            .map(|p| {
                let home = dirs::home_dir().unwrap_or_default();
                p.display()
                    .to_string()
                    .replace(&home.display().to_string(), "~")
            })
            .unwrap_or_else(|_| ".".into());

        let tw: usize = crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80)
            .min(100);

        let inner = tw.saturating_sub(4);

        // ── 1. Hero Image Logo ───────────────────────────────────────
        let logo_path = std::env::current_dir()
            .unwrap_or_default()
            .join("the-cook.png");
        let logo_conf = viuer::Config {
            transparent: true,
            absolute_offset: false,
            x: 2,
            width: Some(28),
            height: Some(14),
            ..Default::default()
        };
        let _ = std::io::stdout().flush();
        if logo_path.exists() {
            if let Err(_e) = viuer::print_from_file(logo_path.to_str().unwrap_or(""), &logo_conf) {
                // Fallback: text-only
            }
        }

        println!();

        // ── 2. Title Bar ─────────────────────────────────────────────
        let title = format!(" HEISENBERG v{} ", ver);
        let title_pad_l = inner.saturating_sub(title.len()) / 2;
        let title_pad_r = inner.saturating_sub(title.len() + title_pad_l);
        println!(
            "  {D}{BOX_TL}{}{BOX_TR}{R}",
            BOX_H.repeat(inner)
        );
        println!(
            "  {D}{BOX_V}{R}{}{ORG}{B}{title}{R}{}{D}{BOX_V}{R}",
            " ".repeat(title_pad_l),
            " ".repeat(title_pad_r),
        );
        println!(
            "  {D}{BOX_T}{}{BOX_TE}{R}",
            BOX_H.repeat(inner)
        );

        // ── 3. Info Section ──────────────────────────────────────────
        let model_name = self.engine.model_name();
        let rss_mb = self.mike.rss_bytes() as f64 / 1_048_576.0;
        let max_mb = self.mike.max_bytes as f64 / 1_048_576.0;
        let ctx = if let Some(plan) = self.engine.active_plan() {
            plan.context_length
        } else {
            8192
        };

        let info_lines: Vec<(&str, String, &str)> = vec![
            ("Model",  model_name,                                  CYN),
            ("RAM",    format!("{:.0} / {:.0} MB", rss_mb, max_mb), GRN),
            ("Ctx",    format!("{} tokens", ctx),                    CYN),
            ("Status", "Online & Ready".to_string(),                 GRN),
            ("Dir",    cwd,                                          BLUE),
        ];

        let label_w = 8; // fixed label width for alignment

        for (label, value, color) in &info_lines {
            let _content = format!("{:>label_w$}  {}", label, value, label_w = label_w);
            let content_len = label_w + 2 + value.len();
            let pad = inner.saturating_sub(content_len + 2);
            println!(
                "  {D}{BOX_V}{R} {WHT}{B}{:>w$}{R}  {color}{value}{R}{}{D}{BOX_V}{R}",
                label,
                " ".repeat(pad.saturating_sub(1)),
                w = label_w
            );
        }

        // ── 4. Separator + Hint ──────────────────────────────────────
        println!(
            "  {D}{BOX_T}{}{BOX_TE}{R}",
            BOX_H.repeat(inner)
        );
        let hint = "Type /help for commands, /q to exit";
        let hint_pad = inner.saturating_sub(hint.len() + 2);
        println!(
            "  {D}{BOX_V}{R} {I}{D}{hint}{R}{}{D}{BOX_V}{R}",
            " ".repeat(hint_pad.saturating_sub(1)),
        );
        println!(
            "  {D}{BOX_BL}{}{BOX_BR}{R}",
            BOX_H.repeat(inner)
        );
        println!();
    }


    // ══════════════════════════════════════════════════════════════════
    //  COMPACT STATUS BAR (inline, always visible)
    // ══════════════════════════════════════════════════════════════════
    fn print_status_bar(&self) {
        let model = self.engine.model_name();
        let short_model = if model.len() > 20 {
            format!("{}…", &model[..19])
        } else {
            model.clone()
        };

        let rss = self.mike.rss_bytes() as f64 / 1_048_576.0;
        let max = self.mike.max_bytes as f64 / 1_048_576.0;
        let pct = self.mike.usage_pct();

        let mem_col = if pct < 50.0 {
            GRN
        } else if pct < 80.0 {
            YLW
        } else {
            RED
        };

        let ctx = if let Some(p) = self.engine.active_plan() { p.context_length } else { 8192 };
        let pressure = self.mike.pressure();
        let status_str = format!("{:?}", pressure);
        let status_col = match pressure {
            crate::mike::MemoryPressure::Normal => GRN,
            crate::mike::MemoryPressure::Elevated => YLW,
            crate::mike::MemoryPressure::High => ORG,
            crate::mike::MemoryPressure::Critical => RED,
        };

        println!(
            "\n  {D}{BOX_TL}─[{CYN}{short_model}{D}]─[{mem_col}{:.0}/{:.0}MB{D}]─[{CYN}ctx:{ctx}{D}]─[{status_col}{status_str}{D}]{R}",
            rss, max
        );
    }

    fn cmd_status(&self) {
        self.mike.monitor.refresh();
        let rss = self.mike.rss_bytes() as f64 / 1_048_576.0;
        let max = self.mike.max_bytes as f64 / 1_048_576.0;
        let pct = self.mike.usage_pct();
        let filled = ((pct / 100.0) * 30.0) as usize;
        let empty = 30usize.saturating_sub(filled);
        let col = if pct < 60.0 {
            GRN
        } else if pct < 80.0 {
            YLW
        } else {
            RED
        };
        let pressure = self.mike.pressure();

        println!();
        println!("  {CYN}{B}⚗  HEISENBERG STATUS{R}");
        println!("  {D}──────────────────────────────────────{R}");
        println!(
            "  {WHT}RAM{R}       {col}{}{R}{D}{}{R}  {WHT}{:.0}/{:.0} MB{R} ({:.1}%)",
            "█".repeat(filled),
            "░".repeat(empty),
            rss,
            max,
            pct
        );
        println!(
            "  {WHT}Pressure{R}  {col}{} {:?}{R}",
            pressure.emoji(),
            pressure
        );
        println!("  {WHT}Model{R}     {CYN}{}{R}", self.engine.model_name());
        println!("  {WHT}Format{R}    {:?}", self.engine.prompt_format());
        println!(
            "  {WHT}Engine{R}    {}",
            if self.engine.is_running() {
                format!("{GRN}● Online{R}")
            } else {
                format!("{D}○ Offline{R}")
            }
        );

        // TurboQuant details
        if let Some(plan) = self.engine.active_plan() {
            println!("  {D}──────────────────────────────────────{R}");
            println!("  {MAG}{B}⚡ TurboQuant Active{R}");
            println!("  {WHT}KV Cache K{R}  {MAG}{}{R}", plan.kv_cache_k_type);
            println!("  {WHT}KV Cache V{R}  {MAG}{}{R}", plan.kv_cache_v_type);
            println!(
                "  {WHT}Context{R}     {MAG}{} tokens{R}",
                plan.context_length
            );
            println!("  {WHT}Batch{R}       {MAG}{}{R}", plan.batch_size);
            println!(
                "  {WHT}Flash Attn{R}  {MAG}{}{R}",
                if plan.flash_attn { "ON" } else { "OFF" }
            );
            println!(
                "  {WHT}MMap{R}        {MAG}{}{R}",
                if plan.mmap { "ON" } else { "OFF" }
            );
            println!(
                "  {WHT}Est. Total{R}  {MAG}{:.0} MB{R} (weights: {:.0} MB + KV: {:.0} MB)",
                plan.estimated_total_mb, plan.weight_mb, plan.kv_cache_mb
            );
        }

        // Zone breakdown
        let zones = &self.mike.zones;
        if zones.total() > 0 {
            println!("  {D}──────────────────────────────────────{R}");
            println!("  {WHT}Zones{R}     {D}{}{R}", zones.report());
        }

        println!("  {WHT}Quota{R}     {YLW}∞{R} Unlimited {D}(local inference){R}");
        println!("  {D}──────────────────────────────────────{R}");
        println!();
    }


    // ══════════════════════════════════════════════════════════════════
    //  EXEC
    // ══════════════════════════════════════════════════════════════════
    async fn cmd_exec(&self, script: String, inline: bool) -> Result<()> {
        let res = if inline {
            self.runner.run_inline(&script).await?
        } else {
            self.runner
                .run_script(std::path::Path::new(&script))
                .await?
        };
        if res.success() {
            println!("{}", res.stdout);
        } else {
            eprintln!(
                "  {RED}✖ Failed{R} (exit {}):\n{}",
                res.exit_code,
                res.combined_output()
            );
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════
    //  CHAT (interactive REPL — Claude Code style)
    // ══════════════════════════════════════════════════════════════════
    async fn cmd_chat(&mut self, init_prompt: Option<String>) -> Result<()> {
        // Show startup progress
        print!("  {D}⚗  Starting inference engine with TurboQuant...{R}");
        std::io::stdout().flush()?;

        self.engine.start(&self.mike).await?;

        // Clear the progress line and show welcome
        print!("\r\x1b[2K");
        self.print_welcome();

        // Print compact status bar
        self.print_status_bar();

        let mut messages = vec![ChatMessage::system(
            crate::the_lab::prompt::PromptBuilder::system_prompt(),
        )];

        if let Some(p) = init_prompt {
            messages.push(ChatMessage::user(p));
            self.generate_and_print(&mut messages).await?;
        }

        // ── Main REPL loop ──
        loop {
            // Claude Code-style prompt: clean two-line design
            print!("\n  {CYN}{B}❯{R} ");
            std::io::stdout().flush()?;

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
                break;
            }
            let input = input.trim();

            // ── Slash commands ──
            match input {
                "/q" | "exit" | "quit" => {
                    self.print_goodbye();
                    break;
                }
                "/status" => {
                    self.cmd_status();
                    continue;
                }
                "/clear" => {
                    messages.truncate(1);
                    println!("  {D}Context cleared.{R}");
                    continue;
                }
                "/turbo" => {
                    self.print_turbo_quant_report();
                    continue;
                }
                "/model" => {
                    println!("  {CYN}{B}Active Model:{R} {}", self.engine.model_name());
                    println!("  {CYN}Format:{R} {:?}", self.engine.prompt_format());
                    continue;
                }
                "/help" | "/commands" | "?" => {
                    self.print_commands();
                    continue;
                }
                s if s.starts_with('/') => {
                    println!("  {YLW}Unknown command:{R} {input}  {D}(try /help){R}");
                    continue;
                }
                _ => {}
            }

            messages.push(ChatMessage::user(input));
            self.mike.enforce_cap()?;
            let old_len = messages.len();
            self.generate_and_print(&mut messages).await?;
            if let Some(db) = &self.db {
                if messages.len() > old_len {
                    let resp = &messages[old_len].content;
                    let _ = db.record_interaction(input, resp, None, None).await;
                }
            }
            self.print_status_bar();
        }

        self.engine.stop().await;
        Ok(())
    }

    async fn generate_and_print(&self, messages: &mut Vec<ChatMessage>) -> Result<()> {
        let max_iters = 10;
        let mut loop_count = 0;
        loop {
            if loop_count >= max_iters {
                println!("  {RED}✖ Exceeded max autonomous iterations ({}).{R}", max_iters);
                break;
            }
            loop_count += 1;

            print!("  {D}⚗  Cooking...{R}");
            use std::io::Write;
            std::io::stdout().flush()?;

            match self.engine.generate(messages, &self.mike).await {
                Ok(resp) => {
                    print!("\r\x1b[2K"); // clear the cooking line
                    // Claude Code-style response: clean indented block
                    println!();
                    for line in resp.lines() {
                        println!("  {WHT}{line}{R}");
                    }
                    println!();

                    messages.push(ChatMessage::assistant(&resp));

                    let mut acted = false;

                    // 1. Bash execution <execute>
                    if let Some(start) = resp.find("<execute>") {
                        if let Some(end) = resp.find("</execute>") {
                            if start < end {
                                 let cmd = &resp[start + 9..end].trim();
                                println!("  {ORG}{B}⚡ Bash:{R} {D}{cmd}{R}");

                                // Queue into Gus scheduler for tracking
                                let mut task = Task::new_inline("agent-exec", *cmd);
                                task.mark_running();

                                match self.runner.run_inline(cmd).await {
                                    Ok(res) => {
                                        println!("  {GRN}✓ Complete{R} {D}({:.0}ms){R}", res.elapsed_ms);
                                        if !res.stdout.trim().is_empty() {
                                            for line in res.stdout.lines().take(20) {
                                                println!("  {D}│{R} {line}");
                                            }
                                        }
                                        messages.push(ChatMessage::system(res.summary()));
                                        task.mark_done(res.summary());
                                        acted = true;
                                    }
                                    Err(e) => {
                                        println!("  {RED}✖ Error:{R} {}", e);
                                        messages.push(ChatMessage::system(format!("Error: {}", e)));
                                        task.mark_failed(e.to_string());
                                        acted = true;
                                    }
                                }
                                let _ = self.scheduler.enqueue(task);
                            }
                        }
                    }

                    // 2. Browser Open <browse>
                    if !acted {
                        if let Some(start) = resp.find("<browse>") {
                            if let Some(end) = resp.find("</browse>") {
                                if start < end {
                                    let url = &resp[start + 8..end].trim();
                                    println!("  {ORG}{B}🌐 Browser:{R} {D}{url}{R}");
                                    let result = self.browser.open(url).await;
                                    let msg = match result {
                                        Ok(_) => {
                                            println!("  {GRN}✓ Navigation complete{R}");
                                            format!("Successfully navigated to {}", url)
                                        }
                                        Err(e) => {
                                            println!("  {RED}✖ Browser error:{R} {}", e);
                                            format!("Error navigating to {}: {}", url, e)
                                        }
                                    };
                                    messages.push(ChatMessage::system(msg));
                                    acted = true;
                                }
                            }
                        }
                    }

                    // 3. Screen Observer Capture <capture_screen>
                    if !acted && resp.contains("<capture_screen>") {
                        println!("  {ORG}{B}👁 Vision:{R} {D}Capturing screen...{R}");
                        let result = self.vision.capture().await;
                        let msg = match result {
                            Ok(path) => {
                                println!("  {GRN}✓ Saved:{R} {D}{}{R}", path.display());
                                format!("Screen captured at {}", path.display())
                            }
                            Err(e) => {
                                println!("  {RED}✖ Vision error:{R} {}", e);
                                format!("Error capturing screen: {}", e)
                            }
                        };
                        messages.push(ChatMessage::system(msg));
                        acted = true;
                    }

                    // 4. File operations (write, edit, delete)
                    if !acted {
                        if let Some(start) = resp.find("<write_file path=\"") {
                            let path_start = start + 18;
                            if let Some(path_end) = resp[path_start..].find("\">") {
                                let path = &resp[path_start..path_start + path_end];
                                let content_start = path_start + path_end + 2;
                                if let Some(end) = resp[content_start..].find("</write_file>") {
                                    let content = &resp[content_start..content_start + end].trim();
                                    println!("  {ORG}{B}📝 Write File:{R} {D}{}{R}", path);
                                    let res = std::fs::write(path, content);
                                    let msg = match res {
                                        Ok(_) => {
                                            println!("  {GRN}✓ File written{R}");
                                            format!("Successfully wrote to {}", path)
                                        }
                                        Err(e) => {
                                            println!("  {RED}✖ Write failed:{R} {}", e);
                                            format!("Failed to write to {}: {}", path, e)
                                        }
                                    };
                                    messages.push(ChatMessage::system(msg));
                                    acted = true;
                                }
                            }
                        } else if let Some(start) = resp.find("<delete_file path=\"") {
                            let path_start = start + 19;
                            if let Some(path_end) = resp[path_start..].find("\"") {
                                let path = &resp[path_start..path_start + path_end];
                                println!("  {ORG}{B}🗑 Delete File:{R} {D}{}{R}", path);
                                let res = std::fs::remove_file(path);
                                let msg = match res {
                                    Ok(_) => {
                                        println!("  {GRN}✓ File deleted{R}");
                                        format!("Successfully deleted {}", path)
                                    }
                                    Err(e) => {
                                        println!("  {RED}✖ Delete failed:{R} {}", e);
                                        format!("Failed to delete {}: {}", path, e)
                                    }
                                };
                                messages.push(ChatMessage::system(msg));
                                acted = true;
                            }
                        } else if let Some(start) = resp.find("<edit_file path=\"") {
                            let path_start = start + 17;
                            if let Some(path_end) = resp[path_start..].find("\"") {
                                let path = &resp[path_start..path_start + path_end];
                                let mode_start = path_start + path_end + 8; // mode="..."
                                if let Some(mode_end) = resp[mode_start..].find("\">") {
                                    let mode_str = &resp[mode_start..mode_start + mode_end];
                                    let content_start = mode_start + mode_end + 2;
                                    if let Some(end) = resp[content_start..].find("</edit_file>") {
                                        let content = &resp[content_start..content_start + end];
                                        println!("  {ORG}{B}📝 Edit File:{R} {D}{}{R}", path);
                                        
                                        // Execute sed or appended via Bash since Bash is our execution engine
                                        let bash_cmd = if mode_str.contains("append") {
                                            format!("echo '{}' >> {}", content.replace('\'', "'\\''"), path)
                                        } else {
                                            // Provide patch or replace instructions back to bash runner
                                            format!("echo 'Edit mode \"{}\" requires Bash implementation' && exit 1", mode_str)
                                        };
                                        
                                        match self.runner.run_inline(&bash_cmd).await {
                                            Ok(res) => {
                                                if res.success() {
                                                    println!("  {GRN}✓ File edited{R}");
                                                    messages.push(ChatMessage::system(format!("Successfully edited {}", path)));
                                                } else {
                                                    println!("  {RED}✖ Edit failed:{R} {}", res.stderr);
                                                    messages.push(ChatMessage::system(format!("Failed to edit {}: {}", path, res.stderr)));
                                                }
                                            }
                                            Err(e) => {
                                                messages.push(ChatMessage::system(format!("Edit error: {}", e)));
                                            }
                                        }
                                        acted = true;
                                    }
                                }
                            }
                        }
                    }

                    // 4. Blue Sky Patches <patch>
                    if !acted {
                        if let Some(start) = resp.find("<patch>") {
                            if let Some(end) = resp.find("</patch>") {
                                if start < end {
                                    let patch_data = &resp[start + 7..end].trim();
                                    println!("  {ORG}{B}⚕ Patching:{R} {D}applying self-modification...{R}");
                                    let result = self.patcher.apply_diff(patch_data).await;
                                    let msg = match result {
                                        Ok(_) => {
                                            println!("  {GRN}✓ Patch applied{R}");
                                            "Successfully applied self-modification patch and passed validation.".to_string()
                                        }
                                        Err(e) => {
                                            println!("  {RED}✖ Patch error:{R} {}", e);
                                            format!("Failed to apply patch or tests failed: {}", e)
                                        }
                                    };
                                    messages.push(ChatMessage::system(msg));
                                    acted = true;
                                }
                            }
                        }
                    }

                    if acted {
                        continue;
                    }

                    break;
                }
                Err(e) => {
                    print!("\r\x1b[2K");
                    error!("Generation failed: {e}");
                    eprintln!("  {RED}{B}✖ Error:{R} {e}");
                    break;
                }
            }
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════
    //  AUTONOMOUS RUN (Gus-powered task execution)
    // ══════════════════════════════════════════════════════════════════
    async fn cmd_run(&mut self, task_description: String, dry_run: bool) -> Result<()> {
        info!("Autonomous task: {}", task_description);

        print!("  {D}⚗  Starting engine for autonomous run...{R}");
        std::io::stdout().flush()?;
        self.engine.start(&self.mike).await?;
        print!("\r\x1b[2K");

        println!("  {MAG}{B}⚡ Autonomous Mode{R}");
        println!("  {D}──────────────────────────────────────{R}");
        println!("  {WHT}Task:{R} {}", task_description);

        if dry_run {
            println!("  {YLW}Dry run — planning only, no execution.{R}");
        }

        // Ask the LLM to decompose the task into steps
        let planning_prompt = format!(
            "Break down this task into concrete executable steps. \
             For each step, provide a bash command in <execute></execute> tags. \
             Be precise and sequential.\n\nTask: {}",
            task_description
        );

        let mut messages = vec![
            ChatMessage::system(crate::the_lab::prompt::PromptBuilder::system_prompt()),
            ChatMessage::user(&planning_prompt),
        ];

        // Get the plan from the LLM
        match self.engine.generate(&messages, &self.mike).await {
            Ok(plan) => {
                println!("\n  {CYN}{B}📋 Plan:{R}");
                for line in plan.lines() {
                    println!("  {D}│{R} {line}");
                }
                println!();

                if dry_run {
                    println!("  {YLW}Dry run complete. No commands executed.{R}");
                    self.engine.stop().await;
                    return Ok(());
                }

                messages.push(ChatMessage::assistant(&plan));

                // Execute the plan through the same action-loop used by chat
                println!("  {GRN}{B}▶ Executing plan...{R}\n");
                self.generate_and_print(&mut messages).await?;

                println!("\n  {GRN}{B}✓ Autonomous task complete.{R}");
            }
            Err(e) => {
                eprintln!("  {RED}✖ Planning failed:{R} {}", e);
            }
        }

        self.engine.stop().await;
        Ok(())
    }



    fn print_turbo_quant_report(&self) {
        println!();
        println!("  {MAG}{B}⚡ TurboQuant Report{R}");
        println!("  {D}──────────────────────────────────────{R}");
        if let Some(plan) = self.engine.active_plan() {
            let label_w = 14;
            println!("  {WHT}{:>w$}{R}  Adaptive KV Cache Quantization", "Strategy", w = label_w);
            println!(
                "  {WHT}{:>w$}{R}  {MAG}{} ({:.0}-bit){R}",
                "K Cache", plan.kv_cache_k_type, plan.kv_quant_bits,
                w = label_w
            );
            println!(
                "  {WHT}{:>w$}{R}  {MAG}{} ({:.0}-bit){R}",
                "V Cache", plan.kv_cache_v_type, plan.kv_quant_bits,
                w = label_w
            );
            println!("  {WHT}{:>w$}{R}  {MAG}{} tokens{R}", "Context", plan.context_length, w = label_w);
            println!("  {WHT}{:>w$}{R}  {MAG}{}{R}", "Batch", plan.batch_size, w = label_w);
            println!(
                "  {WHT}{:>w$}{R}  {MAG}{}{R}",
                "Flash Attn",
                if plan.flash_attn { "Enabled" } else { "Disabled" },
                w = label_w
            );
            println!(
                "  {WHT}{:>w$}{R}  {MAG}{}{R}",
                "MMap",
                if plan.mmap { "Enabled" } else { "Disabled" },
                w = label_w
            );
            println!(
                "  {WHT}{:>w$}{R}  {MAG}{}{R}",
                "MLock",
                if plan.mlock { "Enabled" } else { "Disabled" },
                w = label_w
            );
            println!("  {D}──────────────────────────────────────{R}");
            println!("  {WHT}Estimated Memory Usage:{R}");
            println!("  {WHT}{:>w$}{R}  {:.0} MB", "Weights", plan.weight_mb, w = label_w);
            println!("  {WHT}{:>w$}{R}  {:.0} MB", "KV Cache", plan.kv_cache_mb, w = label_w);
            println!(
                "  {WHT}{:>w$}{R}  {MAG}{:.0} MB{R}",
                "Total", plan.estimated_total_mb,
                w = label_w
            );
            let budget_mb = self.mike.max_bytes as f64 / 1_048_576.0;
            let headroom = budget_mb - plan.estimated_total_mb;
            println!(
                "  {WHT}{:>w$}{R}  {:.0} MB ({GRN}+{:.0} MB headroom{R})",
                "Budget", budget_mb, headroom,
                w = label_w
            );

            // KV compression ratio
            let compression = 16.0 / plan.kv_quant_bits;
            println!("  {D}──────────────────────────────────────{R}");
            println!(
                "  {MAG}{B}KV Compression: {:.1}× vs FP16{R}",
                compression
            );
            println!("  {D}──────────────────────────────────────{R}");
            println!("  {MAG}{B}Native TQ-Prod (QJL) & TQ-MSE Engine loaded{R}");
            println!("  {MAG}{B}via nalgebra & rand_distr in the_lab::turbo_quant{R}");
        } else {
            println!("  {D}Engine not running. Start a chat session first.{R}");
        }
        println!();
    }

    fn print_commands(&self) {
        println!();
        println!("  {D}{BOX_TL}─ {ORG}{B}Commands{R} {D}────────────────────────────────{BOX_TR}{R}");
        println!("  {D}{BOX_V}{R}  {B}/status{R}    Show RAM & system status      {D}{BOX_V}{R}");
        println!("  {D}{BOX_V}{R}  {B}/turbo{R}     TurboQuant report             {D}{BOX_V}{R}");
        println!("  {D}{BOX_V}{R}  {B}/model{R}     Show active model info        {D}{BOX_V}{R}");
        println!("  {D}{BOX_V}{R}  {B}/clear{R}     Clear conversation context    {D}{BOX_V}{R}");
        println!("  {D}{BOX_V}{R}  {B}/help{R}      Show this help                {D}{BOX_V}{R}");
        println!("  {D}{BOX_V}{R}  {B}/q{R}         Exit the session              {D}{BOX_V}{R}");
        println!("  {D}{BOX_BL}────────────────────────────────────────{BOX_BR}{R}");
        println!();
    }

    fn print_goodbye(&self) {
        println!();
        println!("  {D}{I}\"Say my name.\"{R} {D}— Session ended.{R}");
        println!();
    }
}
