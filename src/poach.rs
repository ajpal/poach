use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};

use poach::EGraph;
use poach::ast::Parser as EgglogParser;

use std::fs::File;
use std::io::{prelude::*, stderr};
use std::process::exit;

use flexbuffers::{FlexbufferSerializer, Reader};

use poach::report::{MetricValue, Reporter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(version, about)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Produce a model from a training set
    Train(TrainArgs),
    /// Use a model to process egglog programs
    Serve(ServeArgs),
    /// Update a model with new input-output pairs
    FineTune(FineTuneArgs),
    /// TEST
    Test(TestArgs),
}

#[derive(Debug, Args)]
struct TrainArgs {
    /// If true, prints statistics to stderr
    #[arg(short, long)]
    debug: bool,

    /// Requires a directory or a file
    training_set: PathBuf,

    /// Requires a file
    output_model_file: PathBuf,
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// If true, prints statistics to stderr
    #[arg(short, long)]
    debug: bool,

    /// Requires a file
    model_file: PathBuf,

    #[command(subcommand)]
    mode: ServeMode,
}

/// More subtle distinctions for the serve API
/// As of Apr 9, 26, we are not there yet
#[derive(Debug, Subcommand)]
enum ServeMode {
    /// Open world: Commands are not known in advance and must be processed in order.
    ///
    /// Read input from stdin until terminated by EOF
    /// Prints outputs to stdout as they arise
    Streaming,

    /// Closed world: all required commands are known in advance (in the file)
    /// and can be reordered as desired by the algorithm
    ///
    /// Input: A single .egg file
    /// Outputs get written to stdout
    Single { input_file: PathBuf },

    /// Batch input:
    ///   reads all .egg files in the input directory
    ///   writes outputs files to the output directory
    ///   the order of the input files should not matter
    ///   this means the model only needs to be loaded once for all
    Batch {
        input_dir: PathBuf,
        output_dir: PathBuf,
    },
}

#[derive(Debug, Args)]
struct FineTuneArgs {
    /// If true, prints statistics to stderr
    #[arg(short, long)]
    debug: bool,

    /// Requires a file
    input_model_file: PathBuf,

    /// Requires two folders
    /// Really should be a list of pairs instead of a pair of lists
    /// For now, assumes the filename would relate the input to the output
    input_dir: PathBuf,
    output_dir: PathBuf,

    /// Requires a file
    output_model_file: PathBuf,
}

#[derive(Debug, Args)]
struct TestArgs {}

pub fn poach() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Train(arg) => {
            train(arg);
        }
        Commands::Serve(arg) => {
            serve(arg);
        }
        Commands::FineTune(arg) => {
            fine_tune(arg);
        }
        Commands::Test(arg) => {
            println!("test({:?})", arg);
        }
    }
}

// mut is necessary due to possible canonicalization before serializing
fn serialize_egraph_to_file(egraph: &mut EGraph, output_file: &Path) -> usize {
    egraph.stabilize();

    let mut buf = FlexbufferSerializer::new();
    Serialize::serialize(egraph, &mut buf).expect("Failed to serialize the egraph to Flexbuffer");
    let serialized_size = buf.view().len();

    let Ok(mut file) = File::create(output_file) else {
        panic!("Failed to create file");
    };
    let _ = file.write_all(buf.view());
    serialized_size
}

/// SerializeEgraph assumes a single input egglog program
fn train(arg: TrainArgs) {
    let mut egraph = EGraph::default();

    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();

    let input = arg.training_set;
    if arg.debug {
        let mut reporter = Reporter::new();

        let read_timer = reporter.new_timer("read_egg_file".to_string(), vec!["io".to_string()]);
        let program = std::fs::read_to_string(input.as_path()).expect("Failed to read file");
        reporter.record_timer(read_timer);

        let parsed = egraph
            .parser
            .get_program_from_string(Some(input.to_str().unwrap().into()), &program)
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to execute {:} with error {:?}",
                    input.to_string_lossy(),
                    e
                )
            });

        match egraph.run_program_with_reporter(parsed, &mut reporter) {
            Ok(_) => {
                let serialize_timer =
                    reporter.new_timer("serialize_model".to_string(), vec!["io".to_string()]);
                let serialized_size =
                    serialize_egraph_to_file(&mut egraph, arg.output_model_file.as_path());
                reporter.record_timer(serialize_timer);
                reporter.record_size(
                    "model_bytes".to_string(),
                    MetricValue::Bytes(serialized_size as u64),
                );
                serde_json::to_writer(&mut stderr(), &reporter.build_report())
                    .expect("Failed to serialize report");
                eprintln!();
            }
            Err(e) => {
                panic!(
                    "Failed to execute {:} with error {:?}",
                    input.to_string_lossy(),
                    e
                );
            }
        }
    } else {
        let program = std::fs::read_to_string(input.as_path()).expect("Failed to read file");

        match egraph.parse_and_run_program(Some(input.to_str().unwrap().into()), &program) {
            Ok(_) => {
                let _ = serialize_egraph_to_file(&mut egraph, arg.output_model_file.as_path());
            }
            Err(e) => {
                panic!(
                    "Failed to execute {:} with error {:?}",
                    input.to_string_lossy(),
                    e
                );
            }
        }
    }
}

fn deserialize_egraph_from_file(egraph_file: &Path, reporter: Option<&mut Reporter>) -> EGraph {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();

    if let Some(reporter) = reporter {
        let read_timer = reporter.new_timer("read_model_file".to_string(), vec!["io".to_string()]);
        let Ok(mut file) = File::open(egraph_file) else {
            panic!("Failed to open input egraph file");
        };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .expect("Failed to read from file");
        reporter.record_timer(read_timer);

        let r = Reader::get_root(buf.as_slice()).unwrap();
        let deserialize_timer =
            reporter.new_timer("deserialize_model".to_string(), vec!["io".to_string()]);

        let mut egraph = EGraph::deserialize(r).unwrap();

        egraph
            .restore_deserialized_runtime()
            .expect("Failed to restore deserialized runtime");
        reporter.record_timer(deserialize_timer);

        egraph
    } else {
        let Ok(mut file) = File::open(egraph_file) else {
            panic!("Failed to open input egraph file");
        };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .expect("Failed to read from file");

        let r = Reader::get_root(buf.as_slice()).unwrap();

        let mut egraph = EGraph::deserialize(r).unwrap();

        egraph
            .restore_deserialized_runtime()
            .expect("Failed to restore deserialized runtime");

        egraph
    }
}

fn serve(arg: ServeArgs) {
    match arg.mode {
        ServeMode::Streaming => {
            match deserialize_egraph_from_file(arg.model_file.as_path(), None) // no reporter for streaming mode yet
                .repl(poach::RunMode::Normal)
            {
                Ok(_) => {}
                _ => {
                    exit(-1);
                }
            }
        }
        ServeMode::Single { input_file: input } => {
            if arg.debug {
                let mut reporter = Reporter::new();
                let mut egraph =
                    deserialize_egraph_from_file(arg.model_file.as_path(), Some(&mut reporter));

                let read_timer =
                    reporter.new_timer("read_input_program".to_string(), vec!["io".to_string()]);
                let program =
                    std::fs::read_to_string(input.as_path()).expect("Failed to read file");
                reporter.record_timer(read_timer);

                let mut parser = EgglogParser::default();
                let parsed = parser
                    .get_program_from_string(Some(input.to_str().unwrap().into()), &program)
                    .unwrap_or_else(|e| {
                        panic!(
                            "Failed to execute {:} with error {:?}",
                            input.to_string_lossy(),
                            e
                        )
                    });

                match egraph.run_program_with_reporter(parsed, &mut reporter) {
                    Ok(msgs) => {
                        for msg in msgs {
                            print!("{msg}");
                        }
                        serde_json::to_writer(&mut stderr(), &reporter.build_report())
                            .expect("Failed to serialize report");
                        eprintln!();
                    }
                    Err(e) => {
                        panic!(
                            "Failed to execute {:} with error {:?}",
                            input.to_string_lossy(),
                            e
                        );
                    }
                }
            } else {
                let mut egraph = deserialize_egraph_from_file(arg.model_file.as_path(), None);
                let program =
                    std::fs::read_to_string(input.as_path()).expect("Failed to read file");

                match egraph.parse_and_run_program(Some(input.to_str().unwrap().into()), &program) {
                    Ok(msgs) => {
                        for msg in msgs {
                            print!("{msg}");
                        }
                    }
                    Err(e) => {
                        panic!(
                            "Failed to execute {:} with error {:?}",
                            input.to_string_lossy(),
                            e
                        );
                    }
                }
            }
        }
        ServeMode::Batch {
            input_dir: _,
            output_dir: _,
        } => {
            //TODO
            panic!("Batch not implemented");
        }
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
