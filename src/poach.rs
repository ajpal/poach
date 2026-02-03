use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use egglog::ast::{all_sexps, Sexp, SexpParser};
use egglog::{CommandOutput, EGraph, TimedEgraph};
use env_logger::Env;
use hashbrown::HashMap;
use serde::Serialize;

use std::fmt::{Debug, Display};
use std::fs::{self, create_dir_all, read_to_string, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Debug)]
enum RunMode {
    // For each egg file under the input path,
    //      run the egglog program and record timing information. Do not serialize.
    //      Save the complete timeline, for consumption by the nightly frontend.
    TimelineOnly,

    // For each egg file under the input path,
    //      run the egglog program and record timing information.
    //      Serialize to disk.
    //      Save the complete timeline, for consumption by the nightly frontend.
    Serialize,

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

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information
    //      Round trip to JSON Value, no File I/O
    //      Find all extract commands from the egglog program and perform
    //      the same extractions on the deserialized egraph
    //      Ensure the results are the same
    //      Save the completed timeline, for consumption by the nighly frontend
    Extract,

    // Requires initial-egraph to be provided via Args
    // For each egg file under the input path,
    //      Deserialize the initial egraph
    //      Run the egglog program, skipping declarations of Sorts and Rules
    //      Save the completed timeline, for consumption by the nightly frontend
    Mine,
}

impl Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RunMode::TimelineOnly => "timeline",
                RunMode::SequentialRoundTrip => "sequential",
                RunMode::Serialize => "serialize",
                RunMode::InterleavedRoundTrip => "interleaved",
                RunMode::IdempotentRoundTrip => "idempotent",
                RunMode::OldSerialize => "old-serialize",
                RunMode::NoIO => "no-io",
                RunMode::Extract => "extract",
                RunMode::Mine => "mine",
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

    #[arg(long)]
    initial_egraph: Option<PathBuf>,
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
        serde_json::to_writer_pretty(BufWriter::new(file), &diff)
            .expect("failed to serialize diff");
        panic!("Diff for {}", name)
    }
}

fn run_egg_file(
    initial_egraph: Option<&Path>,
    egg_file: &PathBuf,
) -> Result<(TimedEgraph, Vec<CommandOutput>)> {
    let mut egraph = if let Some(path) = initial_egraph {
        println!("Starting from {}", path.display());
        TimedEgraph::new_from_file(path)
    } else {
        TimedEgraph::new()
    };

    let filename = egg_file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let program_text = read_to_string(egg_file)?;

    let parsed_commands = egraph
        .egraphs
        .last_mut()
        .expect("There are no egraphs")
        .parser
        .get_program_from_string(Some(filename.to_string()), &program_text)?;

    let outputs = egraph.run_program_with_timeline(parsed_commands, &program_text)?;

    Ok((egraph, outputs))
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

fn compare_extracts(
    initial_extracts: &[CommandOutput],
    final_extracts: &[CommandOutput],
) -> Result<()> {
    if initial_extracts.len() != final_extracts.len() {
        anyhow::bail!("extract lengths mismatch")
    }

    for (x, y) in initial_extracts.iter().zip(final_extracts) {
        match (x, y) {
            (CommandOutput::ExtractBest(_, _, term1), CommandOutput::ExtractBest(_, _, term2)) => {
                if term1 != term2 {
                    anyhow::bail!("No match : {:?} {:?}", x, y)
                }
            }
            (
                CommandOutput::ExtractVariants(_, terms1),
                CommandOutput::ExtractVariants(_, terms2),
            ) => {
                if terms1 != terms2 {
                    anyhow::bail!("No match : {:?} {:?}", x, y)
                }
            }
            (
                CommandOutput::MultiExtractVariants(_, items1),
                CommandOutput::MultiExtractVariants(_, items2),
            ) => {
                if items1 != items2 {
                    anyhow::bail!("No match : {:?} {:?}", x, y)
                }
            }
            _ => anyhow::bail!("No match : {:?} {:?}", x, y),
        }
    }

    Ok(())
}

fn poach(
    files: Vec<PathBuf>,
    out_dir: &PathBuf,
    run_mode: RunMode,
    initial_egraph: Option<PathBuf>,
) -> (Vec<String>, Vec<(String, String)>) {
    match run_mode {
        RunMode::TimelineOnly => process_files(&files, out_dir, |egg_file, out_dir| {
            let (egraph, _) = run_egg_file(initial_egraph.as_deref(), egg_file)?;
            egraph.write_timeline(out_dir)?;

            Ok(())
        }),

        RunMode::Serialize => process_files(&files, out_dir, |egg_file, out_dir| {
            let (mut egraph, _) = run_egg_file(initial_egraph.as_deref(), egg_file)?;
            egraph.to_file(&out_dir.join("serialize.json"))?;
            egraph.write_timeline(out_dir)?;
            Ok(())
        }),

        RunMode::SequentialRoundTrip => {
            process_files(&files, out_dir, |egg_file, out_dir: &PathBuf| {
                let (mut egraph, _) = run_egg_file(initial_egraph.as_deref(), egg_file)?;
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
                let (mut egraph, _) = run_egg_file(initial_egraph.as_deref(), egg_file)?;
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
            let (mut egraph, _) = run_egg_file(initial_egraph.as_deref(), &egg_file)?;
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
            let (mut egraph, _) = run_egg_file(initial_egraph.as_deref(), egg_file)?;

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
            let (mut egraph, _) = run_egg_file(initial_egraph.as_deref(), egg_file)?;

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

        RunMode::Extract => process_files(&files, out_dir, |egg_file, out_dir| {
            let (mut timed_egraph, initial_outputs) =
                run_egg_file(initial_egraph.as_deref(), egg_file)?;

            let initial_extracts: Vec<CommandOutput> = initial_outputs
                .into_iter()
                .filter(|x| {
                    matches!(
                        x,
                        CommandOutput::ExtractBest(_, _, _)
                            | CommandOutput::ExtractVariants(_, _)
                            | CommandOutput::MultiExtractVariants(_, _)
                    )
                })
                .collect();

            let program_string = &read_to_string(egg_file)?;

            let is_extract = |sexp: &&Sexp| {
                if let Sexp::List(xs, _) = sexp {
                    if !xs.is_empty() {
                        match &xs[0] {
                            Sexp::Atom(s, _) => s == "extract",
                            _ => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            let all_sexps = all_sexps(SexpParser::new(None, program_string))?;
            let extracts: String = all_sexps
                .iter()
                .filter(is_extract)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join("\n");

            let extract_cmds = timed_egraph
                .egraphs
                .last_mut()
                .expect("there are no egraphs")
                .parser
                .get_program_from_string(None, &extracts)?;

            let value = timed_egraph
                .to_value()
                .context("Failed to encode egraph as JSON")?;

            timed_egraph
                .from_value(value)
                .context("failed to decode egraph from json")?;

            check_egraph_number(&timed_egraph, 2)?;

            let final_extracts = timed_egraph.run_program_with_timeline(extract_cmds, &extracts)?;

            compare_extracts(&initial_extracts, &final_extracts)?;

            timed_egraph.write_timeline(out_dir)?;

            Ok(())
        }),

        RunMode::Mine => {
            assert!(initial_egraph.is_some());
            process_files(&files, out_dir, |egg_file, out_dir| {
                let mut timed_egraph =
                    TimedEgraph::new_from_file(&initial_egraph.as_ref().unwrap());

                let program_string = &read_to_string(egg_file)?;

                let all_sexps = all_sexps(SexpParser::new(None, program_string))?;

                let all_cmds = EGraph::default()
                    .parser
                    .get_program_from_string(None, &program_string)?;

                assert!(all_cmds.len() == all_sexps.len());

                let (filtered_cmds, filtered_sexps): (Vec<_>, Vec<_>) = all_cmds
                    .into_iter()
                    .zip(all_sexps)
                    .filter(|(c, _)| {
                        matches!(
                            c,
                            egglog::ast::GenericCommand::Action(_)
                                | egglog::ast::GenericCommand::Extract(_, _, _)
                                | egglog::ast::GenericCommand::MultiExtract(_, _, _)
                                | egglog::ast::GenericCommand::RunSchedule(_)
                                | egglog::ast::GenericCommand::PrintOverallStatistics
                                | egglog::ast::GenericCommand::Check(_, _)
                                | egglog::ast::GenericCommand::PrintFunction(_, _, _, _, _)
                                | egglog::ast::GenericCommand::PrintSize(_, _)
                        )
                    })
                    .unzip();

                timed_egraph.run_program_with_timeline(
                    filtered_cmds,
                    &filtered_sexps
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )?;

                timed_egraph.write_timeline(out_dir)?;

                Ok(())
            })
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
    let output_dir = args.output_dir.join(args.run_mode.to_string());

    create_dir_all(&output_dir).expect("Failed to create output directory");

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

    let (success, failure) = poach(entries, &output_dir, args.run_mode, args.initial_egraph);
    #[derive(Serialize)]
    struct Output {
        success: Vec<String>,
        failure: Vec<(String, String)>,
    }
    let out = Output { success, failure };
    let file =
        File::create(output_dir.join("summary.json")).expect("Failed to create summary.json");
    serde_json::to_writer_pretty(BufWriter::new(file), &out).expect("failed to write summary.json");
}
