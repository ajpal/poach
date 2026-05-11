use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use poach::EGraph;
use poach::extraction_cache::ExtractionCache;
use poach::report::Reporter;

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

// Assumes a single input egglog program
fn train(arg: TrainArgs) {
    let mut cache = ExtractionCache::new();

    if arg.debug {
        eprintln!("training on {}", arg.training_set.display());
    }
    let mut egraph = EGraph::default();
    let src = std::fs::read_to_string(&arg.training_set).expect("failed to read training file");
    let program = egraph
        .parser
        .get_program_from_string(Some(arg.training_set.display().to_string()), &src)
        .expect("failed to parse training file");
    let mut reporter = Reporter::new();
    egraph
        .run_program_with_reporter_and_cache(program, &mut reporter, &mut cache)
        .expect("error running training file");

    cache
        .save(&arg.output_model_file)
        .expect("failed to write model file");

    if arg.debug {
        eprintln!("wrote cache to {}", arg.output_model_file.display());
    }
}

fn serve(arg: ServeArgs) {
    let mut cache = ExtractionCache::load(&arg.model_file).expect("failed to load model file");

    match arg.mode {
        ServeMode::Single { input_file } => {
            let mut egraph = EGraph::default();
            let src = std::fs::read_to_string(&input_file).expect("failed to read input file");
            let program = egraph
                .parser
                .get_program_from_string(Some(input_file.display().to_string()), &src)
                .expect("failed to parse input file");
            let mut reporter = Reporter::new();
            let outputs = egraph
                .run_program_with_reporter_and_cache(program, &mut reporter, &mut cache)
                .expect("error running input file");
            for output in outputs {
                print!("{}", output);
            }
            if arg.debug {
                eprintln!("served {}", input_file.display());
            }
        }
        ServeMode::Streaming => {
            eprintln!("serve --streaming is not yet implemented");
        }
        ServeMode::Batch { .. } => {
            eprintln!("serve --batch is not yet implemented");
        }
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
