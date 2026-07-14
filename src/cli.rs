//! CLI mode — run configs headlessly from terminal.
//!
//! Usage:
//!   ironbullet --config <path> --wordlist <path> [options]
//!
//! Supports .rfx (native), .svb (SilverBullet), .opk (OpenBullet), and .loli (LoliCode) formats.

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::export::format::RfxConfig;
use crate::import::import_config_bytes;
use crate::pipeline::Pipeline;
use crate::runner::data_pool::DataPool;
use crate::runner::proxy_pool::ProxyPool;
use crate::runner::{HitResult, RunnerOrchestrator, RunnerSetup};
use crate::sidecar::native::create_native_backend;

pub struct CliArgs {
    pub config: String,
    pub wordlist: String,
    pub threads: Option<u32>,
    pub proxies: Option<String>,
    pub outfile: Option<String>,
    pub skip: Option<u32>,
    pub take: Option<u32>,
    pub debug: bool,
}

pub fn parse_args(args: &[String]) -> Result<CliArgs, String> {
    let mut config = None;
    let mut wordlist = None;
    let mut threads = None;
    let mut proxies = None;
    let mut outfile = None;
    let mut skip = None;
    let mut take = None;
    let mut debug = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                config = Some(args.get(i).ok_or("--config requires a path")?.clone());
            }
            "--wordlist" | "-w" => {
                i += 1;
                wordlist = Some(args.get(i).ok_or("--wordlist requires a path")?.clone());
            }
            "--threads" | "-t" => {
                i += 1;
                let val = args.get(i).ok_or("--threads requires a number")?;
                threads = Some(
                    val.parse::<u32>()
                        .map_err(|_| format!("invalid thread count: {}", val))?,
                );
            }
            "--proxies" | "-p" => {
                i += 1;
                proxies = Some(args.get(i).ok_or("--proxies requires a path")?.clone());
            }
            "--outfile" | "-o" => {
                i += 1;
                outfile = Some(args.get(i).ok_or("--outfile requires a path")?.clone());
            }
            "--skip" => {
                i += 1;
                let val = args.get(i).ok_or("--skip requires a number")?;
                skip = Some(
                    val.parse::<u32>()
                        .map_err(|_| format!("invalid skip value: {}", val))?,
                );
            }
            "--take" => {
                i += 1;
                let val = args.get(i).ok_or("--take requires a number")?;
                take = Some(
                    val.parse::<u32>()
                        .map_err(|_| format!("invalid take value: {}", val))?,
                );
            }
            "--debug" | "-d" => {
                debug = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                return Err(format!("unknown argument: {}", other));
            }
        }
        i += 1;
    }

    Ok(CliArgs {
        config: config.ok_or("--config is required")?,
        wordlist: wordlist.ok_or("--wordlist is required")?,
        threads,
        proxies,
        outfile,
        skip,
        take,
        debug,
    })
}

fn print_help() {
    eprintln!(
        "ironbullet — pipelined request automation

USAGE:
  ironbullet --config <path> --wordlist <path> [options]

OPTIONS:
  -c, --config <path>     Config file (.rfx, .svb, .opk, .loli, .json)
  -w, --wordlist <path>   Wordlist / combo file
  -t, --threads <n>       Thread count (overrides config)
  -p, --proxies <path>    Proxy list file
  -o, --outfile <dir>     Output directory for hits (default: results/)
      --skip <n>          Skip first N data lines
      --take <n>          Process only N data lines (0 = all)
  -d, --debug             Print each block result to stderr
  -h, --help              Show this help"
    );
}

/// Load a config from any supported format, returns the pipeline
fn load_config(path: &str) -> Result<Pipeline, String> {
    // Try native .rfx first
    if path.ends_with(".rfx") {
        let rfx = RfxConfig::load_from_file(path)
            .map_err(|e| format!("failed to load {}: {}", path, e))?;
        return Ok(rfx.pipeline);
    }

    // Otherwise read bytes and auto-detect
    let bytes = std::fs::read(path).map_err(|e| format!("failed to read {}: {}", path, e))?;
    let result = import_config_bytes(&bytes)?;

    if !result.warnings.is_empty() {
        for w in &result.warnings {
            eprintln!("[warn] {}", w);
        }
    }
    if !result.security_issues.is_empty() {
        for issue in &result.security_issues {
            eprintln!(
                "[{:?}] {} — {}",
                issue.severity, issue.title, issue.description
            );
        }
    }

    Ok(result.pipeline)
}

/// Entry point for CLI mode — blocks until the run completes
pub async fn run(cli: CliArgs) -> Result<(), String> {
    // Load config
    let mut pipeline = load_config(&cli.config)?;
    eprintln!(
        "[*] loaded config: {} ({} blocks)",
        pipeline.name,
        pipeline.blocks.len()
    );

    // Apply CLI overrides
    if let Some(t) = cli.threads {
        pipeline.runner_settings.threads = t;
    }
    if let Some(s) = cli.skip {
        pipeline.runner_settings.skip = s;
    }
    if let Some(t) = cli.take {
        pipeline.runner_settings.take = t;
    }
    if let Some(ref dir) = cli.outfile {
        pipeline.output_settings.output_directory = dir.clone();
        pipeline.output_settings.save_to_file = true;
    }

    let threads = pipeline.runner_settings.threads as usize;
    let skip = pipeline.runner_settings.skip as usize;
    let take = pipeline.runner_settings.take as usize;

    // Load data
    let mut data_pool = DataPool::from_file(&cli.wordlist, true)
        .map_err(|e| format!("failed to load wordlist: {}", e))?;
    let total = data_pool.total();

    // Apply skip/take by rebuilding pool
    if skip > 0 || take > 0 {
        let lines: Vec<String> = {
            let mut all = Vec::with_capacity(total);
            while let Some((line, _)) = data_pool.next_line() {
                all.push(line);
            }
            let skipped = all.into_iter().skip(skip);
            if take > 0 {
                skipped.take(take).collect()
            } else {
                skipped.collect()
            }
        };
        data_pool = DataPool::new(lines);
    }

    eprintln!(
        "[*] wordlist: {} lines (skip={}, take={})",
        data_pool.total(),
        skip,
        take
    );

    // Load proxies
    let proxy_pool = if let Some(ref proxy_path) = cli.proxies {
        let pp = ProxyPool::from_file(proxy_path, pipeline.proxy_settings.ban_duration_secs)
            .map_err(|e| format!("failed to load proxies: {}", e))?;
        eprintln!("[*] proxies: {} loaded", pp.total());
        pp
    } else {
        ProxyPool::empty()
    };

    // Create native HTTP backend (no sidecar needed)
    let sidecar_tx = create_native_backend();

    // Hits channel
    let (hits_tx, mut hits_rx) = mpsc::channel::<HitResult>(1024);

    let debug = cli.debug;
    let proxy_mode = pipeline.proxy_settings.proxy_mode.clone();

    let orchestrator = Arc::new(RunnerOrchestrator::new(RunnerSetup {
        pipeline,
        proxy_mode,
        data_pool,
        proxy_pool,
        sidecar_tx,
        thread_count: threads,
        hits_tx,
        plugin_manager: None,
        chrome_executable_path: None,
        custom_input_values: std::collections::HashMap::new(),
    }));

    eprintln!("[*] starting with {} threads", threads);

    // Spawn stats reporter
    let orch_stats = orchestrator.clone();
    let stats_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if !orch_stats.is_running() {
                break;
            }
            let s = orch_stats.get_stats();
            eprint!(
                "\r[*] {}/{} | hits:{} fails:{} errors:{} | cpm:{:.0} | threads:{}   ",
                s.processed, s.total, s.hits, s.fails, s.errors, s.cpm, s.active_threads
            );
        }
    });

    // Spawn hit printer
    let hit_handle = tokio::spawn(async move {
        let mut count = 0u64;
        while let Some(hit) = hits_rx.recv().await {
            count += 1;
            let caps: String = hit
                .captures
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" | ");
            if caps.is_empty() {
                println!("[HIT] {}", hit.data_line);
            } else {
                println!("[HIT] {} | {}", hit.data_line, caps);
            }
            if debug {
                if let Some(ref p) = hit.proxy {
                    eprintln!("  proxy: {}", p);
                }
            }
        }
        count
    });

    // Run
    orchestrator.start().await;

    // Final stats
    let stats = orchestrator.get_stats();
    eprintln!(
        "\r[*] done in {:.1}s — {} processed, {} hits, {} fails, {} errors",
        stats.elapsed_secs, stats.processed, stats.hits, stats.fails, stats.errors
    );

    stats_handle.abort();
    let hit_count = hit_handle.await.unwrap_or(0);
    if hit_count > 0 {
        eprintln!("[*] {} hits printed to stdout", hit_count);
    }

    Ok(())
}
