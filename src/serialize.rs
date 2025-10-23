// entry point for POACH v0
use anyhow::{Context, Result};
use clap::Parser;
use egglog::{run_commands, EGraph, RunMode};
use env_logger::Env;
use serde_json::json;
use std::path::PathBuf;
use std::{
    fs,
    io::{self, BufReader},
    path::Path,
};
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(version = env!("FULL_VERSION"), about = env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    /// Turns off the seminaive optimization
    #[clap(long)]
    naive: bool,

    /// The file names for the egglog files to run
    inputs: Vec<PathBuf>,
}

fn serialize_egraph(egraph: &EGraph, path: &Path) -> Result<()> {
    let file = fs::File::create(path)
        .with_context(|| format!("failed to create file {}", path.display()))?;
    serde_json::to_writer_pretty(file, egraph)
        .with_context(|| format!("failed to serialize egraph to {}", path.display()))?;
    Ok(())
}

fn deserialize_egraph(path: &Path) -> Result<EGraph> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open file {}", path.display()))?;
    let reader = BufReader::new(file);
    let egraph = serde_json::from_reader(reader)?;
    Ok(egraph)
}

pub fn poach_all() {
    let args = Args::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .parse_default_env()
        .init();

    assert!(args.inputs.len() == 1);
    let input_path = &args.inputs[0];

    let out_dir = PathBuf::from("out");
    fs::create_dir_all(&out_dir).expect("failed to create out dir");

    let mut successes = Vec::new();
    let mut failures = Vec::new();

    let entries: Vec<PathBuf> = if input_path.is_file() {
        if input_path.extension().and_then(|s| s.to_str()) == Some("egg") {
            vec![input_path.clone()]
        } else {
            panic!("Input file is not an .egg file");
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

    for (i, path) in entries.iter().enumerate() {
        if path.extension().and_then(|s| s.to_str()) != Some("egg") {
            continue;
        }

        let file_out_dir = out_dir.join(path.file_stem().unwrap().to_str().unwrap());
        fs::create_dir_all(&file_out_dir).expect("fail");

        println!("[{}/{}] Processing {}", i, entries.len(), path.display());
        let name = format!("{}", path.display());
        match poach_one(&path, &file_out_dir, &args) {
            Ok(n) => successes.push(format!("{} ({})", name, n)),
            Err(e) => {
                println!("{:?}", e);
                failures.push(format!("{} [{}]", name, e))
            }
        }
    }
    let v = json!({"success": successes, "fail": failures});
    serde_json::to_writer_pretty(fs::File::create("summary.json").expect("fail"), &v)
        .expect("fail");
}

fn poach_one(path: &PathBuf, out_dir: &PathBuf, args: &Args) -> Result<usize> {
    let mut egraph = EGraph::default();

    egraph.seminaive = !args.naive;

    let program = std::fs::read_to_string(path).expect("failed to open");
    match run_commands(
        &mut egraph,
        Some(path.to_str().unwrap().into()),
        &program,
        io::stdout(),
        RunMode::NoMessages, // silence output to keep logs small
    ) {
        Ok(None) => {}
        _ => anyhow::bail!("[{}]run_commands failed", path.display()),
    }

    let s1 = out_dir.join("serialize1.json");
    let s2 = out_dir.join("serialize2.json");
    let s3 = out_dir.join("serialize3.json");
    serialize_egraph(&egraph, &s1).context("failed to write s1.json")?;

    let e2 = deserialize_egraph(&s1)?;
    serialize_egraph(&e2, &s2).context("failed to write s2.json")?;

    let e3 = deserialize_egraph(&s2)?;
    serialize_egraph(&e3, &s3).context("failed to write s3.json")?;

    let e2_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&s2).context("couldn't open serialize2.json")?)
            .context("couldn't parse serialize2 as json")?;

    let e3_contents = fs::read_to_string(&s3).context("couldn't open serialize3.json")?;
    let e3_len = e3_contents.len();
    let e3_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&s3).context("couldn't open serialize3.json")?)
            .context("couldn't parse serialize3 as json")?;

    match egraph.num_tuples() == e3.num_tuples() {
        true => {}
        false => {
            anyhow::bail!(
                "Started with {} tuples, ended with {}",
                egraph.num_tuples(),
                e3.num_tuples()
            )
        }
    }

    match serde_json_diff::values(e2_json, e3_json) {
        Some(diff) => {
            let file = fs::File::create(out_dir.join("diff.json"))
                .with_context(|| format!("failed to create diff file for {}", path.display()))?;
            serde_json::to_writer_pretty(file, &diff)
                .with_context(|| format!("failed to serialize diff to {}", path.display()))?;
            anyhow::bail!("diff for {}", path.display())
        }
        None => Ok(e3_len),
    }
}

fn main() {
    poach_all();
}
