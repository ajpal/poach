use anyhow::{Context, Result};
use clap::Parser;
use linux_perf_data::linux_perf_event_reader::EventRecord;
use linux_perf_data::{PerfFileReader, PerfFileRecord};
use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(about = "Analyze perf.data files and emit a JSON summary")]
struct Args {
    /// Directory containing *.perf.data files
    perf_dir: PathBuf,

    /// Output JSON path
    #[arg(long, default_value = "nightly/output/perf/perf-analysis-stage3.json")]
    out: PathBuf,

    /// Root function substring (e.g. run_extract_command).
    #[arg(long)]
    root_symbol: Option<String>,

    /// Callee function substrings to track within root_symbol. Can be passed multiple times.
    #[arg(long = "callee-symbol")]
    callee_symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PerfFileSummary {
    file: String,
    sample_record_count: usize,
    parsed_event_record_count: usize,
    function_focus: Option<FunctionFocusSummary>,
}

#[derive(Debug, Serialize)]
struct CalleeMatchSummary {
    symbol: String,
    sample_record_count: usize,
    period_sum: u64,
}

#[derive(Debug, Serialize)]
struct FunctionFocusSummary {
    root_symbol: String,
    root_sample_record_count: usize,
    root_period_sum: u64,
    callees: Vec<CalleeMatchSummary>,
}

#[derive(Debug, Serialize)]
struct PerfSummary {
    perf_dir: String,
    root_symbol: Option<String>,
    callee_symbols: Vec<String>,
    total_files: usize,
    total_sample_records: usize,
    total_parsed_event_records: usize,
    files: Vec<PerfFileSummary>,
}

#[derive(Debug)]
struct FunctionMatchCounter {
    symbol: String,
    sample_record_count: usize,
    period_sum: u64,
}

fn find_perf_files(perf_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(perf_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into_path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with(".perf.data"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

fn count_sample_records(perf_data_path: &Path) -> Result<(usize, usize)> {
    let file = File::open(perf_data_path)
        .with_context(|| format!("failed to open {}", perf_data_path.display()))?;
    let reader = BufReader::new(file);

    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)
        .with_context(|| format!("failed to parse {}", perf_data_path.display()))?;

    let mut sample_record_count = 0usize;
    let mut parsed_event_record_count = 0usize;

    loop {
        let next_record = record_iter
            .next_record(&mut perf_file)
            .with_context(|| format!("failed to read record from {}", perf_data_path.display()))?;

        let Some(perf_record) = next_record else {
            break;
        };

        let PerfFileRecord::EventRecord { record, .. } = perf_record else {
            continue;
        };

        parsed_event_record_count += 1;
        if let EventRecord::Sample(_) = record.parse().with_context(|| {
            format!(
                "failed to parse event record in {}",
                perf_data_path.display()
            )
        })? {
            sample_record_count += 1;
        }
    }

    Ok((sample_record_count, parsed_event_record_count))
}

fn parse_period_from_header(header_line: &str) -> u64 {
    let Some(colon_idx) = header_line.find(':') else {
        return 1;
    };
    let after_ts = header_line[colon_idx + 1..].trim_start();
    let first_token = after_ts.split_whitespace().next().unwrap_or("1");
    first_token.parse::<u64>().unwrap_or(1)
}

fn parse_symbol_from_stack_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let _ip = parts.next()?;
    let symbol_with_offset = parts.next()?;
    let symbol = symbol_with_offset
        .split("+0x")
        .next()
        .unwrap_or(symbol_with_offset)
        .to_string();
    Some(symbol)
}

fn finalize_sample_block(
    symbols: &[String],
    period: u64,
    root_symbol: &str,
    callee_counters: &mut [FunctionMatchCounter],
    root_sample_record_count: &mut usize,
    root_period_sum: &mut u64,
) {
    if symbols.is_empty() {
        return;
    }

    let root_present = symbols.iter().any(|s| s.contains(root_symbol));
    if !root_present {
        return;
    }

    *root_sample_record_count += 1;
    *root_period_sum += period;

    for callee in callee_counters.iter_mut() {
        if symbols.iter().any(|s| s.contains(&callee.symbol)) {
            callee.sample_record_count += 1;
            callee.period_sum += period;
        }
    }
}

fn analyze_function_focus_with_perf_script(
    perf_data_path: &Path,
    root_symbol: &str,
    callee_symbols: &[String],
) -> Result<FunctionFocusSummary> {
    let mut child = Command::new("perf")
        .arg("script")
        .arg("-i")
        .arg(perf_data_path)
        .arg("--demangle")
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run perf script for {}", perf_data_path.display()))?;

    let stdout = child
        .stdout
        .take()
        .context("failed to capture perf script stdout")?;
    let reader = BufReader::new(stdout);

    let mut current_symbols: Vec<String> = Vec::new();
    let mut current_period = 1u64;
    let mut in_sample = false;

    let mut root_sample_record_count = 0usize;
    let mut root_period_sum = 0u64;
    let mut callee_counters: Vec<FunctionMatchCounter> = callee_symbols
        .iter()
        .cloned()
        .map(|symbol| FunctionMatchCounter {
            symbol,
            sample_record_count: 0,
            period_sum: 0,
        })
        .collect();

    for line in reader.lines() {
        let line = line.with_context(|| {
            format!(
                "failed to read perf script output for {}",
                perf_data_path.display()
            )
        })?;

        if line.trim().is_empty() {
            if in_sample {
                finalize_sample_block(
                    &current_symbols,
                    current_period,
                    root_symbol,
                    &mut callee_counters,
                    &mut root_sample_record_count,
                    &mut root_period_sum,
                );
                current_symbols.clear();
                current_period = 1;
                in_sample = false;
            }
            continue;
        }

        if !in_sample {
            current_period = parse_period_from_header(&line);
            in_sample = true;
            continue;
        }

        if let Some(symbol) = parse_symbol_from_stack_line(&line) {
            current_symbols.push(symbol);
        }
    }

    if in_sample {
        finalize_sample_block(
            &current_symbols,
            current_period,
            root_symbol,
            &mut callee_counters,
            &mut root_sample_record_count,
            &mut root_period_sum,
        );
    }

    let status = child.wait().with_context(|| {
        format!(
            "failed waiting for perf script ({})",
            perf_data_path.display()
        )
    })?;
    if !status.success() {
        anyhow::bail!(
            "perf script failed for {} with status {}",
            perf_data_path.display(),
            status
        );
    }

    Ok(FunctionFocusSummary {
        root_symbol: root_symbol.to_string(),
        root_sample_record_count,
        root_period_sum,
        callees: callee_counters
            .into_iter()
            .map(|c| CalleeMatchSummary {
                symbol: c.symbol,
                sample_record_count: c.sample_record_count,
                period_sum: c.period_sum,
            })
            .collect(),
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.root_symbol.is_none() && !args.callee_symbols.is_empty() {
        anyhow::bail!("--callee-symbol requires --root-symbol");
    }

    let perf_files = find_perf_files(&args.perf_dir);

    let mut out_files = Vec::with_capacity(perf_files.len());
    let mut total_sample_records = 0usize;
    let mut total_parsed_event_records = 0usize;

    for perf_file in perf_files {
        let (sample_record_count, parsed_event_record_count) = count_sample_records(&perf_file)?;
        total_sample_records += sample_record_count;
        total_parsed_event_records += parsed_event_record_count;

        let function_focus = if let Some(root_symbol) = args.root_symbol.as_deref() {
            Some(analyze_function_focus_with_perf_script(
                &perf_file,
                root_symbol,
                &args.callee_symbols,
            )?)
        } else {
            None
        };

        out_files.push(PerfFileSummary {
            file: perf_file.display().to_string(),
            sample_record_count,
            parsed_event_record_count,
            function_focus,
        });
    }

    let summary = PerfSummary {
        perf_dir: args.perf_dir.display().to_string(),
        root_symbol: args.root_symbol.clone(),
        callee_symbols: args.callee_symbols.clone(),
        total_files: out_files.len(),
        total_sample_records,
        total_parsed_event_records,
        files: out_files,
    };

    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create output directory {}",
                parent.as_os_str().to_string_lossy()
            )
        })?;
    }

    let output_file = File::create(&args.out)
        .with_context(|| format!("failed to create output file {}", args.out.display()))?;
    let mut writer = BufWriter::new(output_file);
    serde_json::to_writer_pretty(&mut writer, &summary)
        .with_context(|| format!("failed to serialize JSON to {}", args.out.display()))?;
    writer.flush().context("failed to flush output file")?;

    println!(
        "Wrote perf analysis summary for {} files to {}",
        summary.total_files,
        args.out.display()
    );
    Ok(())
}
