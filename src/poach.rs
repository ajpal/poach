use std::{fs::File, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use poach::{
    EGraph,
    report::{MetricValue, Reporter},
};

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

fn train(arg: TrainArgs) {
    // Extremely basic placeholder: Just creates an empty file
    if arg.debug {
        let mut reporter = Reporter::new();
        let timer = reporter.new_timer("train".to_string(), vec![]);
        let _ = File::create(arg.output_model_file.as_path());
        reporter.record_timer(timer);

        serde_json::to_writer(&mut std::io::stderr(), &reporter.build_report())
            .expect("Failed to serialize report");
    } else {
        let _ = File::create(arg.output_model_file.as_path());
    }
}

fn serve(arg: ServeArgs) {
    // Extremely basic placeholder: Just creates an empty egraph
    if arg.debug {
        let mut reporter = Reporter::new();
        let timer = reporter.new_timer("serve".to_string(), vec![]);
        let egraph = EGraph::default();
        reporter.record_size(
            "egraph size".to_string(),
            MetricValue::Count(egraph.num_tuples() as u64),
        );
        reporter.record_timer(timer);
        serde_json::to_writer(&mut std::io::stderr(), &reporter.build_report())
            .expect("Failed to serialize report");
    } else {
        let _ = EGraph::default();
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
