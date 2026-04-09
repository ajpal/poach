use poach::{EGraph};

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

    /// None (default): Streaming mode
    ///   reads input from stdin
    ///   terminate when EOF, which is dynamic
    ///   prints output to stdout
    #[command(subcommand)]
    serve_command: Option<ServeCommands>,
}

/// More subtle distinctions for the serve API
/// As of Apr 9, 26, we are not there yet
#[derive(Debug, Subcommand)]
enum ServeCommands {
    /// Single File input:
    ///   reads a single .egg file
    ///   which means it is closed
    ///   prints output to stdout
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
fn train(arg : TrainArgs) {
    let _ = File::create(arg.output_model_file.as_path());
}

/// VanillaEgglog
fn serve(arg: ServeArgs) {
    match arg.serve_command {
        None => {
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
        Some(cmd) => {
            match cmd {
                ServeCommands::Single{input_file: input} => {
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
                ServeCommands::Batch{input_dir:_, output_dir:_} => {
                    //TODO
                    panic!("Batch not implemented");
                }
            }
        }
    }
}

/// VanillaEgglog's model is just unit
/// Still, it would create an empty file
fn fine_tune(arg: FineTuneArgs) {
    let _ = File::create(arg.output_model_file.as_path());
}
