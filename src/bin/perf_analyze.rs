use anyhow::{Context, Result};
use clap::Parser;
use linux_perf_data::linux_perf_event_reader::{EventRecord, SamplingPolicy};
use linux_perf_data::{PerfFileReader, PerfFileRecord};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(about = "Analyze perf.data files and emit benchmark/suite perf summaries")]
struct Args {
    /// Directory containing *.perf.data files
    perf_dir: PathBuf,

    /// Output JSON path
    #[arg(long, default_value = "nightly/output/perf/perf-summary.json")]
    out: PathBuf,

    /// Root function substring (e.g. run_extract_command)
    #[arg(long)]
    root_symbol: String,

    /// Callee function substrings to track within root_symbol. Repeat for multiple values.
    #[arg(long = "callee-symbol")]
    callee_symbols: Vec<String>,
}

#[derive(Debug, Clone)]
struct FunctionMatchCounter {
    sample_record_count: u64,
    period_sum: u64,
}

#[derive(Debug, Clone)]
struct FunctionFocusCounter {
    root_sample_record_count: u64,
    root_period_sum: u64,
    callees: Vec<FunctionMatchCounter>,
}

#[derive(Debug, Clone)]
struct FileMeta {
    event_name: Option<String>,
    sample_freq_hz: Option<u64>,
    sampling_policy: String,
}

#[derive(Debug, Serialize)]
struct MetricSummary {
    sample_record_count: u64,
    period_sum: u64,
    estimated_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
struct CalleeSummary {
    symbol: String,
    sample_record_count: u64,
    period_sum: u64,
    estimated_ms: Option<f64>,
    percent_of_root_samples: f64,
    percent_of_root_period: f64,
}

#[derive(Debug, Serialize)]
struct BenchmarkSummary {
    suite: String,
    benchmark: String,
    file: String,
    sample_record_count: u64,
    parsed_event_record_count: u64,
    root: MetricSummary,
    callees: Vec<CalleeSummary>,
}

#[derive(Debug, Serialize)]
struct SuiteSummary {
    benchmark_count: u64,
    root: MetricSummary,
    callees: Vec<CalleeSummary>,
}

#[derive(Debug, Serialize)]
struct MetaSummary {
    perf_dir: String,
    event_name: Option<String>,
    sample_freq_hz: Option<u64>,
    sampling_policy: Option<String>,
    root_symbol: String,
    callee_symbols: Vec<String>,
    total_files: u64,
}

#[derive(Debug, Serialize)]
struct PerfSummary {
    meta: MetaSummary,
    suites: BTreeMap<String, SuiteSummary>,
    benchmarks: BTreeMap<String, BenchmarkSummary>,
}

#[derive(Debug, Clone)]
struct SuiteAccumulator {
    benchmark_count: u64,
    root_samples: u64,
    root_period: u64,
    callee_samples: Vec<u64>,
    callee_periods: Vec<u64>,
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

fn benchmark_identity(perf_dir: &Path, perf_data_path: &Path) -> Result<(String, String, String)> {
    let rel = perf_data_path.strip_prefix(perf_dir).with_context(|| {
        format!(
            "{} is not under {}",
            perf_data_path.display(),
            perf_dir.display()
        )
    })?;

    let mut rel_components = rel.components();
    let suite = rel_components
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .context("failed to parse suite name from perf file path")?
        .to_string();

    let file_name = rel
        .file_name()
        .and_then(|n| n.to_str())
        .context("failed to parse perf file name")?;
    let benchmark_base = file_name.strip_suffix(".perf.data").unwrap_or(file_name);

    let rel_parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let mut benchmark_segments = Vec::new();
    for component in rel_parent.components().skip(1) {
        if let Some(seg) = component.as_os_str().to_str() {
            if !seg.is_empty() {
                benchmark_segments.push(seg.to_string());
            }
        }
    }
    benchmark_segments.push(benchmark_base.to_string());

    let benchmark = benchmark_segments.join("/");
    let benchmark_key = format!("{suite}/{benchmark}");
    Ok((suite, benchmark, benchmark_key))
}

fn sampling_policy_to_parts(policy: SamplingPolicy) -> (Option<u64>, String) {
    match policy {
        SamplingPolicy::Frequency(hz) => (Some(hz), "frequency".to_string()),
        SamplingPolicy::Period(period) => (None, format!("period({})", period.get())),
        SamplingPolicy::NoSampling => (None, "none".to_string()),
    }
}

fn count_sample_records_and_meta(perf_data_path: &Path) -> Result<(u64, u64, FileMeta)> {
    let file = File::open(perf_data_path)
        .with_context(|| format!("failed to open {}", perf_data_path.display()))?;
    let reader = BufReader::new(file);

    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)
        .with_context(|| format!("failed to parse {}", perf_data_path.display()))?;

    let meta = if let Some(attr) = perf_file.event_attributes().first() {
        let (sample_freq_hz, sampling_policy) = sampling_policy_to_parts(attr.attr.sampling_policy);
        FileMeta {
            event_name: attr.name().map(ToString::to_string),
            sample_freq_hz,
            sampling_policy,
        }
    } else {
        FileMeta {
            event_name: None,
            sample_freq_hz: None,
            sampling_policy: "unknown".to_string(),
        }
    };

    let mut sample_record_count = 0u64;
    let mut parsed_event_record_count = 0u64;

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
        if matches!(
            record.parse().with_context(|| {
                format!(
                    "failed to parse event record in {}",
                    perf_data_path.display()
                )
            })?,
            EventRecord::Sample(_)
        ) {
            sample_record_count += 1;
        }
    }

    Ok((sample_record_count, parsed_event_record_count, meta))
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
    Some(
        symbol_with_offset
            .split("+0x")
            .next()
            .unwrap_or(symbol_with_offset)
            .to_string(),
    )
}

fn finalize_sample_block(
    symbols: &[String],
    period: u64,
    root_symbol: &str,
    callee_symbols: &[String],
    counters: &mut FunctionFocusCounter,
) {
    if symbols.is_empty() {
        return;
    }

    let root_positions: Vec<usize> = symbols
        .iter()
        .enumerate()
        .filter_map(|(idx, sym)| {
            if sym.contains(root_symbol) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    if root_positions.is_empty() {
        return;
    }

    counters.root_sample_record_count += 1;
    counters.root_period_sum += period;

    for (callee_idx, callee) in callee_symbols.iter().enumerate() {
        let callee_under_root = symbols.iter().enumerate().any(|(frame_idx, sym)| {
            if !sym.contains(callee) {
                return false;
            }
            root_positions.iter().any(|root_idx| frame_idx < *root_idx)
        });

        if callee_under_root {
            counters.callees[callee_idx].sample_record_count += 1;
            counters.callees[callee_idx].period_sum += period;
        }
    }
}

fn analyze_function_focus_with_perf_script(
    perf_data_path: &Path,
    root_symbol: &str,
    callee_symbols: &[String],
) -> Result<FunctionFocusCounter> {
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

    let mut counters = FunctionFocusCounter {
        root_sample_record_count: 0,
        root_period_sum: 0,
        callees: callee_symbols
            .iter()
            .map(|_| FunctionMatchCounter {
                sample_record_count: 0,
                period_sum: 0,
            })
            .collect(),
    };

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
                    callee_symbols,
                    &mut counters,
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
            callee_symbols,
            &mut counters,
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

    Ok(counters)
}

fn estimate_ms(samples: u64, sample_freq_hz: Option<u64>) -> Option<f64> {
    sample_freq_hz
        .filter(|hz| *hz > 0)
        .map(|hz| samples as f64 * 1000.0 / hz as f64)
}

fn safe_percent(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        (num as f64 * 100.0) / den as f64
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let perf_files = find_perf_files(&args.perf_dir);

    let mut benchmarks = BTreeMap::new();
    let mut suites_acc: BTreeMap<String, SuiteAccumulator> = BTreeMap::new();

    let mut first_event_name: Option<String> = None;
    let mut first_sample_freq_hz: Option<u64> = None;
    let mut first_sampling_policy: Option<String> = None;

    for perf_file in perf_files.iter() {
        let (suite, benchmark, benchmark_key) = benchmark_identity(&args.perf_dir, perf_file)?;
        let (sample_record_count, parsed_event_record_count, file_meta) =
            count_sample_records_and_meta(perf_file)?;
        let focus = analyze_function_focus_with_perf_script(
            perf_file,
            &args.root_symbol,
            &args.callee_symbols,
        )?;

        if first_event_name.is_none() {
            first_event_name = file_meta.event_name.clone();
        }
        if first_sample_freq_hz.is_none() {
            first_sample_freq_hz = file_meta.sample_freq_hz;
        }
        if first_sampling_policy.is_none() {
            first_sampling_policy = Some(file_meta.sampling_policy.clone());
        }

        let root = MetricSummary {
            sample_record_count: focus.root_sample_record_count,
            period_sum: focus.root_period_sum,
            estimated_ms: estimate_ms(focus.root_sample_record_count, first_sample_freq_hz),
        };

        let mut callee_summaries = Vec::with_capacity(args.callee_symbols.len());
        for (idx, callee_symbol) in args.callee_symbols.iter().enumerate() {
            let callee_counter = &focus.callees[idx];
            callee_summaries.push(CalleeSummary {
                symbol: callee_symbol.clone(),
                sample_record_count: callee_counter.sample_record_count,
                period_sum: callee_counter.period_sum,
                estimated_ms: estimate_ms(callee_counter.sample_record_count, first_sample_freq_hz),
                percent_of_root_samples: safe_percent(
                    callee_counter.sample_record_count,
                    focus.root_sample_record_count,
                ),
                percent_of_root_period: safe_percent(
                    callee_counter.period_sum,
                    focus.root_period_sum,
                ),
            });
        }

        benchmarks.insert(
            benchmark_key,
            BenchmarkSummary {
                suite: suite.clone(),
                benchmark,
                file: perf_file.display().to_string(),
                sample_record_count,
                parsed_event_record_count,
                root,
                callees: callee_summaries,
            },
        );

        let suite_acc = suites_acc.entry(suite).or_insert_with(|| SuiteAccumulator {
            benchmark_count: 0,
            root_samples: 0,
            root_period: 0,
            callee_samples: vec![0; args.callee_symbols.len()],
            callee_periods: vec![0; args.callee_symbols.len()],
        });
        suite_acc.benchmark_count += 1;
        suite_acc.root_samples += focus.root_sample_record_count;
        suite_acc.root_period += focus.root_period_sum;
        for idx in 0..args.callee_symbols.len() {
            suite_acc.callee_samples[idx] += focus.callees[idx].sample_record_count;
            suite_acc.callee_periods[idx] += focus.callees[idx].period_sum;
        }
    }

    let mut suites = BTreeMap::new();
    for (suite_name, acc) in suites_acc {
        let root = MetricSummary {
            sample_record_count: acc.root_samples,
            period_sum: acc.root_period,
            estimated_ms: estimate_ms(acc.root_samples, first_sample_freq_hz),
        };
        let mut callee_summaries = Vec::with_capacity(args.callee_symbols.len());
        for (idx, symbol) in args.callee_symbols.iter().enumerate() {
            callee_summaries.push(CalleeSummary {
                symbol: symbol.clone(),
                sample_record_count: acc.callee_samples[idx],
                period_sum: acc.callee_periods[idx],
                estimated_ms: estimate_ms(acc.callee_samples[idx], first_sample_freq_hz),
                percent_of_root_samples: safe_percent(acc.callee_samples[idx], acc.root_samples),
                percent_of_root_period: safe_percent(acc.callee_periods[idx], acc.root_period),
            });
        }
        suites.insert(
            suite_name,
            SuiteSummary {
                benchmark_count: acc.benchmark_count,
                root,
                callees: callee_summaries,
            },
        );
    }

    let summary = PerfSummary {
        meta: MetaSummary {
            perf_dir: args.perf_dir.display().to_string(),
            event_name: first_event_name,
            sample_freq_hz: first_sample_freq_hz,
            sampling_policy: first_sampling_policy,
            root_symbol: args.root_symbol,
            callee_symbols: args.callee_symbols,
            total_files: perf_files.len() as u64,
        },
        suites,
        benchmarks,
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
        "Wrote perf summary for {} files to {}",
        summary.meta.total_files,
        args.out.display()
    );
    Ok(())
}
