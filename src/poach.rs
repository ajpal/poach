use anyhow::{Context, Result};
use clap::Parser;
use egglog::{EGraph, SerializeConfig, TimedEgraph};
use env_logger::Env;

use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(version = env!("FULL_VERSION"), about= env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    input_path: PathBuf,
    output_path: PathBuf,
    #[arg(long)]
    no_serialize: bool, // temporary flag to turn off serialization round trip because it can be too slow.
}

fn check_egraph_size(egraph: &TimedEgraph) -> Result<()> {
    let expected = egraph.num_tuples();
    for (i, eg) in egraph.egraphs().iter().enumerate() {
        if eg.num_tuples() != expected {
            anyhow::bail!(
                "Egraph {} had {} tuples (expected {})",
                i,
                eg.num_tuples(),
                expected
            )
        }
    }
    Ok(())
}

fn check_idempotent(p1: &PathBuf, p2: &PathBuf, name: &str, out_dir: &PathBuf) -> Result<()> {
    let json1: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(p1).context(format!("failed to open {}", p1.display()))?,
    )
    .context(format!("failed to parse {}", p1.display()))?;

    let json2: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(p2).context(format!("failed to open {}", p2.display()))?,
    )
    .context(format!("failed to parse {}", p2.display()))?;

    match serde_json_diff::values(json1, json2) {
        Some(diff) => {
            let file = fs::File::create(out_dir.join("diff.json"))
                .context("Failed to create diff file")?;
            serde_json::to_writer_pretty(file, &diff).context("failed to serialize diff")?;
            anyhow::bail!("diff for {}", name)
        }
        None => Ok(()),
    }
}

fn old_serialize(egraph: &EGraph, path: PathBuf) -> std::io::Result<()> {
    let serialized_output = egraph.serialize(SerializeConfig::default());

    if serialized_output.is_complete() {
        serialized_output.egraph.to_json_file(path)
    } else {
        let parent = path.parent().unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy();
        let ext = path.extension().unwrap_or_default().to_string_lossy();

        println!("{stem} incomplete");

        serialized_output
            .egraph
            .to_json_file(parent.join(format!("{stem}-incomplete.{ext}")))
    }
}

fn run_one(path: &PathBuf, out_dir: &PathBuf, serialize: bool) -> Result<TimedEgraph> {
    if path.extension().and_then(|s| s.to_str()) != Some("egg") {
        panic!("Not an egg file");
    }

    // Create subdirectory for this file
    let out_dir = out_dir.join(path.file_stem().unwrap().to_str().unwrap());
    fs::create_dir_all(&out_dir).expect("fail");

    // filename for display
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Create TimedEgraph
    let mut egraph: TimedEgraph = TimedEgraph::new();

    // Read egg file
    let program =
        std::fs::read_to_string(path).expect(&format!("Failed to open {}", path.display()));

    // Run
    egraph.parse_and_run_program(filename, &program)?;

    if serialize {
        // Round trip serialize egraph
        let s1 = out_dir.join("serialize1.json");
        let s2 = out_dir.join("serialize2.json");
        let s3 = out_dir.join("serialize3.json");

        old_serialize(egraph.egraphs().last().unwrap(), out_dir.join("old.json"))
            .context("failed to serialize using old")?;

        egraph
            .serialize_egraph(&s1)
            .context("failed to serialize s1.json")?;
        egraph
            .deserialize_egraph(&s1)
            .context("failed to read s1.json")?;

        egraph
            .serialize_egraph(&s2)
            .context("failed to serialize s2.json")?;
        egraph
            .deserialize_egraph(&s2)
            .context("failed to read s2.json")?;

        egraph
            .serialize_egraph(&s3)
            .context("failed to serialize s3.json")?;
        egraph
            .deserialize_egraph(&s3)
            .context("failed to read s3.json")?;

        // Check properties of serialization
        check_egraph_size(&egraph)?;
        check_idempotent(&s2, &s3, filename, &out_dir)?;
        // todo: compare extracts between e1 and e3?
    }

    // Serialize Timeline
    let timeline = egraph.serialized_timeline()?;
    fs::write(out_dir.join("timeline.json"), timeline).context("failed to write timeline.json")?;

    Ok(egraph)
}

fn run_all(files: Vec<PathBuf>, args: Args) {
    let out_dir = args.output_path;
    fs::create_dir_all(&out_dir).expect("failed to create out dir");
    for (i, path) in files.iter().enumerate() {
        let name = format!("{}", path.display());
        let out_path = out_dir.join(path.file_stem().unwrap().to_str().unwrap());

        // Run the timed egraph + serialization
        let res = run_one(&path, &out_dir, !args.no_serialize);

        if let Ok(_) = res {
            println!("[{}/{}] {} - SUCCESS", i, files.len(), name)
        } else {
            println!("[{}/{}] {} - FAIL", i, files.len(), name);
        }
    }
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .parse_default_env()
        .init();
    let input_path = args.input_path.clone();

    let entries = if input_path.is_file() {
        if input_path.extension().and_then(|s| s.to_str()) == Some("egg") {
            vec![input_path]
        } else {
            panic!("input file is not an egg file")
        }
    } else if input_path.is_dir() {
        WalkDir::new(input_path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| !entry.path().to_string_lossy().contains("fail"))
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("egg"))
            .map(|entry| entry.path().to_path_buf())
            .collect()
    } else {
        panic!("Input path is neither file nor directory: {:?}", input_path);
    };
    run_all(entries, args);
}
