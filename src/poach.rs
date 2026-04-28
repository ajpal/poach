use poach::EGraph;

use std::{fs::File, path::PathBuf, process::exit};

use clap::{Args, Parser, Subcommand};

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
            eprintln!("test({:?})", arg);
            //TODO: run vanilla egglog tests
        }
    }
    // TODO handle report IO
}

/// VanillaEgglog's model is just unit
/// Still, it would create an empty file
fn train(arg: TrainArgs) {
    let _ = File::create(arg.output_model_file.as_path());
}

/// VanillaEgglog
fn serve(arg: ServeArgs) {
    match arg.mode {
        ServeMode::Streaming => {
            let mut egraph = EGraph::default();

            rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build_global()
                .unwrap();

            match egraph.repl(poach::RunMode::Normal) {
                Ok(_) => {}
                _ => {
                    exit(-1);
                }
            }
        }
        ServeMode::Single { input_file: input } => {
            let mut egraph = EGraph::default();

            rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build_global()
                .unwrap();

            let program = std::fs::read_to_string(input.as_path()).unwrap_or_else(|_| {
                let arg = input.to_string_lossy();
                panic!("Failed to read file {arg}")
            });

            match egraph.parse_and_run_program(Some(input.to_str().unwrap().into()), &program) {
                Ok(msgs) => {
                    for msg in msgs {
                        print!("{msg}");
                    }
                }
                _ => {
                    exit(-1);
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

/// VanillaEgglog's model is just unit
/// Still, it would create an empty file
fn fine_tune(arg: FineTuneArgs) {
    let _ = File::create(arg.output_model_file.as_path());
}
