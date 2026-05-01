use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};

use poach::ast::GenericCommand;
use poach::{EGraph, Value};

use std::fs::File;
use std::io::prelude::*;
use std::process::exit;

use flexbuffers::{FlexbufferSerializer, Reader};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Model {
    egraph: EGraph,
    cache: Vec<((String, Value), String)>,
}

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
    // TODO handle report IO
}

// TODO add report events

fn serialize_model_to_file(model: &Model, output_file: &Path) {
    let mut buf = FlexbufferSerializer::new();
    Serialize::serialize(model, &mut buf).expect("Failed to serialize the model to Flexbuffer");

    let Ok(mut file) = File::create(output_file) else {
        panic!("Failed to create file");
    };
    let _ = file.write_all(buf.view());
}

fn train(arg: TrainArgs) {
    let mut egraph = EGraph::default();

    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();

    let input = arg.training_set;

    let program = std::fs::read_to_string(input.as_path()).unwrap_or_else(|_| {
        let arg = input.to_string_lossy();
        panic!("Failed to read file {arg}")
    });

    match egraph.parse_and_run_program(Some(input.to_str().unwrap().into()), &program) {
        Ok(_) => {
            egraph.stabilize();
            let cache = egraph.build_extraction_cache();
            let model = Model { egraph, cache };
            serialize_model_to_file(&model, arg.output_model_file.as_path());
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

fn deserialize_model_from_file(model_file: &Path) -> Model {
    let Ok(mut file) = File::open(model_file) else {
        panic!("Failed to open input model file");
    };
    let mut buf = Vec::new();
    let Ok(_) = file.read_to_end(&mut buf) else {
        panic!("Failed to read from file");
    };

    let r = Reader::get_root(buf.as_slice()).unwrap();

    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();

    let mut model = Model::deserialize(r).unwrap();

    let Ok(_) = model.egraph.restore_deserialized_runtime() else {
        panic!("Failed to restore deserialized runtime");
    };

    model
}

fn serve(arg: ServeArgs) {
    let Model { mut egraph, cache } = deserialize_model_from_file(arg.model_file.as_path());

    match arg.mode {
        ServeMode::Streaming => match egraph.repl(poach::RunMode::Normal) {
            Ok(_) => {}
            _ => {
                exit(-1);
            }
        },
        ServeMode::Single { input_file: input } => {
            let program = std::fs::read_to_string(input.as_path()).unwrap_or_else(|_| {
                let arg = input.to_string_lossy();
                panic!("Failed to read file {arg}")
            });

            let cache_map: std::collections::HashMap<(String, Value), String> =
                cache.into_iter().collect();

            let filename = input.to_str().unwrap().to_string();
            let commands = egraph
                .parser
                .get_program_from_string(Some(filename.clone()), &program)
                .unwrap_or_else(|e| {
                    panic!("Failed to parse {filename}: {e:?}");
                });

            for cmd in commands {
                if let GenericCommand::Extract(_, expr, variants) = &cmd {
                    let n_val = egraph
                        .eval_expr(variants)
                        .expect("failed to evaluate variants count");
                    let n: i64 = egraph.value_to_base(n_val.1);

                    // n == 0 is egglog's "extract the single best term" case;
                    // n > 0 asks for n variants, which the cache doesn't store.
                    if n == 0 {
                        let (sort, value) = egraph
                            .eval_expr(expr)
                            .expect("failed to evaluate extract expr");
                        let canon = egraph.get_canonical_value(value, &sort);
                        if let Some(cached) = cache_map.get(&(sort.name().to_owned(), canon)) {
                            println!("{cached}");
                            continue;
                        }
                    }
                }
                match egraph.run_program(vec![cmd]) {
                    Ok(msgs) => {
                        for msg in msgs {
                            print!("{msg}");
                        }
                    }
                    Err(e) => {
                        panic!("Failed to execute command: {e:?}");
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
