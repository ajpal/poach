use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use egglog::TimedEgraph;
use env_logger::Env;
use hashbrown::HashMap;
use serde::Serialize;

use std::fmt::{Debug, Display};
use std::fs::{self, create_dir_all, read_to_string, File};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Debug)]
enum RunMode {
    // For each egg file under the input path,
    //      run the egglog program and record timing information. Do not serialize.
    //      Save the complete timeline, for consumption by the nightly frontend.
    TimelineOnly,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph, recording timing information.
    //      Deserialize the serialized egraph, recording timing information.
    //      Assert the deserialized egraph has the same size as the initial egraph
    //      Save the complete timeline, for consumption by the nightly frontend.
    SequentialRoundTrip,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph
    // For each egg file under the input path,
    //      Deserialize the deserialized egraph
    //      Assert the deserialized egraph has the same size as the initial egraph
    //      Save the complete timeline, for consumption by the nightly frontend.
    InterleavedRoundTrip,

    // For each egg file under the input path,
    //      Run the egglog program.
    //      Round trip to file twice.
    //      Assert that the second round trip is idempotent (though the first may not be), crash if not.
    IdempotentRoundTrip,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph using both the poach serialization code and
    //      the visualizer serialization code, which serializes only the parent-child relationships
    //      Save the complete timeline, for consumption by the nightly frontend.
    OldSerialize,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Round trip to JSON Value, but do not read/write from file
    //      Assert the deserialized egraph has hthe same size as the initial egraph.
    //      Save the completed timeline, for consumption by the nightly frontend
    NoIO,
}

impl Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RunMode::TimelineOnly => "timeline",
                RunMode::SequentialRoundTrip => "sequential",
                RunMode::InterleavedRoundTrip => "interleaved",
                RunMode::IdempotentRoundTrip => "idempotent",
                RunMode::OldSerialize => "old-serialize",
                RunMode::NoIO => "no-io",
            }
        )
    }
}

#[derive(Debug, Parser)]
#[command(version = env!("FULL_VERSION"), about= env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    input_path: PathBuf,
    output_dir: PathBuf,
    run_mode: RunMode,
}

fn check_egraph_number(egraph: &TimedEgraph, expected: usize) -> Result<()> {
    if egraph.egraphs().len() != expected {
        anyhow::bail!(
            "Expected {} egraphs, found {}",
            expected,
            egraph.egraphs().len()
        );
    }
    Ok(())
}

fn check_egraph_size(egraph: &TimedEgraph) -> Result<()> {
    let expected = egraph.num_tuples();
    for eg in egraph.egraphs().iter() {
        if eg.num_tuples() != expected {
            anyhow::bail!("Expected {} tuples, found {}", expected, eg.num_tuples());
        }
    }
    Ok(())
}

fn check_idempotent(p1: &PathBuf, p2: &PathBuf, name: &str, out_dir: &PathBuf) {
    let json1: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(p1).expect(&format!("failed to open {}", p1.display())),
    )
    .expect(&format!("failed to parse {}", p1.display()));

    let json2: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(p2).expect(&format!("failed to open {}", p2.display())),
    )
    .expect(&format!("failed to parse {}", p2.display()));

    if let Some(diff) = serde_json_diff::values(json1, json2) {
        let file = fs::File::create(out_dir.join("diff.json")).expect("Failed to create diff file");
        serde_json::to_writer_pretty(file, &diff).expect("failed to serialize diff");
        panic!("Diff for {}", name)
    }
}

fn run_egg_file(egg_file: &PathBuf) -> TimedEgraph {
    let mut egraph = TimedEgraph::new();
    let filename = egg_file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    egraph
        .parse_and_run_program(
            filename,
            &read_to_string(egg_file).expect(&format!("Failed to open {}", egg_file.display())),
        )
        .expect("fail");

    egraph
}

fn process_files<F>(
    files: &[PathBuf],
    out_dir: &PathBuf,
    mut f: F,
) -> (Vec<String>, Vec<(String, String)>)
where
    F: FnMut(&PathBuf, &PathBuf) -> Result<()>,
{
    let mut failures = vec![];
    let mut successes = vec![];
    for (idx, file) in files.iter().enumerate() {
        let name = file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let out_dir = out_dir.join(file.file_stem().unwrap().to_str().unwrap());

        create_dir_all(&out_dir).expect("Failed to create out dir");

        match f(file, &out_dir) {
            Ok(_) => {
                successes.push(name.to_string());
                println!("[{}/{}] {} : SUCCESS", idx, files.len(), name)
            }
            Err(e) => {
                failures.push((name.to_string(), format!("{}", e)));
                println!("[{}/{}] {} : FAILURE {}", idx, files.len(), name, e)
            }
        }
    }
    if failures.len() == 0 {
        println!("0 failures out of {} files", files.len());
    } else {
        println!("{} failures out of {} files", failures.len(), files.len());
        for (name, reason) in failures.iter() {
            println!("{} | {}", name, reason);
        }
    }
    (successes, failures)
}

fn poach(
    files: Vec<PathBuf>,
    out_dir: &PathBuf,
    run_mode: RunMode,
) -> (Vec<String>, Vec<(String, String)>) {
    match run_mode {
        RunMode::TimelineOnly => process_files(&files, out_dir, |egg_file, out_dir| {
            let egraph = run_egg_file(egg_file);
            egraph.write_timeline(out_dir)?;

            Ok(())
        }),

        RunMode::SequentialRoundTrip => {
            process_files(&files, out_dir, |egg_file, out_dir: &PathBuf| {
                let mut egraph = run_egg_file(egg_file);
                let s1 = out_dir.join("serialize1.json");

                egraph.to_file(&s1).context("Failed to write s1.json")?;

                egraph.from_file(&s1).context("failed to read s1.json")?;

                check_egraph_number(&egraph, 2)?;

                check_egraph_size(&egraph)?;

                egraph.write_timeline(out_dir)?;
                Ok(())
            })
        }

        RunMode::InterleavedRoundTrip => {
            let mut tmp = HashMap::new();
            process_files(&files, out_dir, |egg_file, out_dir| {
                let mut egraph = run_egg_file(egg_file);
                let s1 = out_dir.join("serialize1.json");
                egraph.to_file(&s1).context("Failed to write s1.json")?;
                tmp.insert(egg_file.clone(), (out_dir.clone(), egraph));
                Ok(())
            });
            process_files(&files, out_dir, |egg_file, _| {
                let (out_dir, egraph) = tmp.get_mut(egg_file).unwrap();
                egraph
                    .from_file(&out_dir.join("serialize1.json"))
                    .context("Failed to read s1.json")?;

                check_egraph_number(&egraph, 2)?;
                check_egraph_size(&egraph)?;

                egraph.write_timeline(out_dir)?;
                Ok(())
            })
        }

        RunMode::IdempotentRoundTrip => process_files(&files, out_dir, |egg_file, out_dir| {
            let name = egg_file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let mut egraph = run_egg_file(&egg_file);
            let s1 = out_dir.join("serialize1.json");
            let s2 = out_dir.join("serialize2.json");
            let s3 = out_dir.join("serialize3.json");

            egraph.to_file(&s1).context("failed to serialize s1.json")?;

            egraph.from_file(&s1).context("failed to read s1.json")?;

            egraph.to_file(&s2).context("failed to serialize s2.json")?;

            egraph.from_file(&s2).context("failed to read s2.json")?;

            egraph.to_file(&s3).context("failed to serialize s3.json")?;

            egraph.from_file(&s3).context("failed to read s3.json")?;

            check_egraph_number(&egraph, 4)?;
            check_egraph_size(&egraph)?;
            check_idempotent(&s2, &s3, name, &out_dir);

            egraph.write_timeline(out_dir)?;
            Ok(())
        }),

        RunMode::OldSerialize => process_files(&files, out_dir, |egg_file, out_dir| {
            let mut egraph = run_egg_file(egg_file);

            egraph
                .to_file(&out_dir.join("serialize-poach.json"))
                .context("failed to write poach.json")?;

            egraph
                .old_serialize_egraph(&out_dir.join("serialize-old.json"))
                .context("Failed to serialize old.json")?;

            egraph.write_timeline(out_dir)?;
            Ok(())
        }),

        RunMode::NoIO => process_files(&files, out_dir, |egg_file, out_dir| {
            let mut egraph = run_egg_file(egg_file);

            let value = egraph
                .to_value()
                .context("Failed to encode egraph as json")?;

            egraph
                .from_value(value)
                .context("failed to decode egraph from json")?;

            check_egraph_number(&egraph, 2)?;

            check_egraph_size(&egraph)?;

            egraph.write_timeline(out_dir)?;

            Ok(())
        }),
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
    let output_dir = args.output_dir.join(args.run_mode.to_string());

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

    let (success, failure) = poach(entries, &output_dir, args.run_mode);
    #[derive(Serialize)]
    struct Output {
        success: Vec<String>,
        failure: Vec<(String, String)>,
    }
    let out = Output { success, failure };
    let file =
        File::create(output_dir.join("summary.json")).expect("Failed to create summary.json");
    serde_json::to_writer_pretty(file, &out).expect("failed to write summary.json");
}
