// entry point for POACH v0
use anyhow::{Context, Result};
use clap::Parser;
use egglog::ast::{Command, GenericCommand};
use egglog::{CommandOutput, EGraph};
use env_logger::Env;
use hashbrown::HashMap;
use serde_json::json;
use std::fmt::Debug;
use std::path::PathBuf;
use std::{fs, io::BufReader, path::Path};
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

    let mut successes = Vec::new();
    let mut failures = Vec::new();
    let mut extracts = HashMap::new();

    if input_path.is_file() {
        if input_path.extension().and_then(|s| s.to_str()) == Some("egg") {
            let name = format!("{}", input_path.display());
            println!("Processing single file {}", name);
            match poach_one(&input_path) {
                Ok((egraph, extracts1, extracts2)) => {
                    successes.push(format!("{} ({})", name, egraph.num_tuples()));
                    extracts.insert(
                        name,
                        (format!("{:?}", extracts1), format!("{:?}", extracts2)),
                    );
                }
                Err(e) => {
                    println!("{:?}", e);
                    failures.push(format!("{} [{}]", name, e))
                }
            }
        } else {
            panic!("Input file is not an .egg file");
        }
    } else if input_path.is_dir() {
        let entries: Vec<PathBuf> = WalkDir::new(input_path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| !entry.path().to_string_lossy().contains("fail"))
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("egg"))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for (i, path) in entries.iter().enumerate() {
            let name = format!("{}", path.display());
            println!("[{}/{}] Processing {}", i, entries.len(), name);
            match poach_one(&path) {
                Ok((egraph, extracts1, extracts2)) => {
                    successes.push(format!("{} ({})", name, egraph.num_tuples()));
                    extracts.insert(
                        name,
                        (format!("{:?}", extracts1), format!("{:?}", extracts2)),
                    );
                }
                Err(e) => {
                    println!("{:?}", e);
                    failures.push(format!("{} [{}]", name, e))
                }
            }
        }
    } else {
        panic!("Input path is neither file nor directory: {:?}", input_path);
    }

    let v = json!({"success": successes, "fail": failures, "extracts": extracts});
    serde_json::to_writer_pretty(fs::File::create("summary.json").expect("fail"), &v)
        .expect("fail");
}

fn poach_one(path: &PathBuf) -> Result<(EGraph, Vec<CommandOutput>, Vec<CommandOutput>)> {
    let args = Args::parse();

    let out_dir = PathBuf::from("out");
    fs::create_dir_all(&out_dir).expect("failed to create out dir");

    if path.extension().and_then(|s| s.to_str()) != Some("egg") {
        panic!("Not an egg file");
    }

    let file_out_dir = out_dir.join(path.file_stem().unwrap().to_str().unwrap());
    fs::create_dir_all(&file_out_dir).expect("fail");

    let mut egraph = EGraph::default();

    egraph.seminaive = !args.naive;

    let program = std::fs::read_to_string(path).expect("failed to open");
    let filename = path.to_str().unwrap().into();
    let parsed_program = egraph
        .parser
        .get_program_from_string(Some(filename), &program)?;

    egraph.run_program(parsed_program.clone())?;

    let s1 = out_dir.join("serialize1.json");
    let s2 = out_dir.join("serialize2.json");
    let s3 = out_dir.join("serialize3.json");
    serialize_egraph(&egraph, &s1).context("failed to write s1.json")?;

    let e2 = deserialize_egraph(&s1)?;
    serialize_egraph(&e2, &s2).context("failed to write s2.json")?;

    let mut e3 = deserialize_egraph(&s2)?;
    serialize_egraph(&e3, &s3).context("failed to write s3.json")?;

    let e2_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&s2).context("couldn't open serialize2.json")?)
            .context("couldn't parse serialize2 as json")?;

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

    let (extracts1, extracts2) = compare_extracts(&mut egraph, &mut e3, parsed_program)?;

    match serde_json_diff::values(e2_json, e3_json) {
        Some(diff) => {
            let file = fs::File::create(out_dir.join("diff.json"))
                .with_context(|| format!("failed to create diff file for {}", path.display()))?;
            serde_json::to_writer_pretty(file, &diff)
                .with_context(|| format!("failed to serialize diff to {}", path.display()))?;
            anyhow::bail!("diff for {}", path.display())
        }
        None => Ok((e3, extracts1, extracts2)),
    }
}

fn compare_extracts(
    initial_egraph: &mut EGraph,
    end_egraph: &mut EGraph,
    parsed_program: Vec<Command>,
) -> Result<(Vec<CommandOutput>, Vec<CommandOutput>)> {
    let extracts: Vec<Command> = parsed_program
        .into_iter()
        .filter(|c| match c {
            GenericCommand::Extract(..) => true,
            _ => false,
        })
        .collect();
    let r1 = initial_egraph.run_program(extracts.clone())?;
    let r2 = end_egraph.run_program(extracts.clone())?;
    Ok((r1, r2))
}

fn main() {
    poach_all();
}
