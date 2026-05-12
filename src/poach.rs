use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use hashbrown::HashMap;

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

// The model is a cache tracking the extractions that were computed in the training set and what the results were
// There are two caches, one for `best` and one for `variants`.
// It is currently the case that `extract-variants 1` and `extract` have the same behavior,
// but that is not part of the egglog interface, so we should not depend on it being true.
// Model:
// BestCache: HashMap<String, String>
// VariantsCache: HashMap<String, (Vec<String>, bool)> (keep track of `exhausted`, see notes below)

// Notes on cache lookup:
// Multi-extracts should be decomposed into individual extractions for the purposes of the cache
// cache hit if all extracts in a multi-extract are present, cache miss if any are absent (compute the whole multi-extract in that case)
// When building the cache, extract variants should keep track of how many
// variants were requested and how many were found. If found < requested, that
// means that's all the variants there are in the egraph
// Extract variants should hit if we have at least the number of variants requested
// Miss if we don't have enough variants
// Unless `exhausted` is true, then just return all of the variants we have in the cache

// Question: in train, `extract-variants 1` populate the `best` cache too? Probably not, for the same reasonig as above
// Question: in serve, should `extract-variants 1` check the `best` cache if the `variants` cache is a miss? Probably yes.

pub fn poach() {
    let cli = Cli::parse();

    // initialize thread pool

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
    if arg.debug {
        // Metrics
        // * time for each command
        // * time to serialize
        // * Number of keys in each cache
        // * Number of variants for each key in variants cache
        // * size of egraph (num_tuples)
        // * size of serialized model (bytes)

        // read and parse program

        // make a new egraph + reporter
        // call run_program_with_reporter to run each command + record timing info
        // returns a Vec<CommandOutput>

        // extract_cmds = Vec<Command> filtered down to just Extract, MultiExtract
        // extract_outs = Vec<CommandOutput> filtered down to just
        // ExtractBest, ExtractVariants, MultiExtractVariants
        // Should match up exactly

        // Traverse pairwise and construct caches (Best and Variants)

        // serialize caches as one JSON object:
        // {best: {...}, variants: {...}}
    } else {
        // No reporting overhead

        // Same logic as above, but call run_program not run_program_with_reporter
    }
}

fn serve(arg: ServeArgs) {
    match arg.mode {
        ServeMode::Streaming => todo!("not yet implemented"),
        ServeMode::Single { input_file } => {
            if arg.debug {
                // Metrics
                // * time to deserialize model
                // * time for each command (including checking the cache for extracts)
                // * size of egraph (could be 0)

                // Deserialize model into two caches: `best` and `variants`

                // read and parse program

                // We can't easily use run_program_with_reporter because we
                // want to check the cache first
                // Option 1: Pass cache into run_program_with_reporter (and run_program for non-debug version)
                // Option 2: Basically inline run_program_with_reporter here so we can check the caches first
                // Option 3: Preprocess program commands to look for all extract commands first,
                // and check for cache hits. Call run_program_with_reporter for the other commands
                // and then splice the results together

                // I think we should do Option 3.
                // If there are no outputs other than extracts that are cached,
                // we can skip making an egraph entirely, which is easiest to capitalize on in Option 3.

                // Command types that produce outputs:
                // RunSchedule, PrintOverallStatistics, Extract, MultiExtract,
                // PrintFunction, PrintSize, UserDefined

                // Find the commands that should produce outputs
                // Find the extractions (a subset of the above)
                // Find hits/misses in the caches
                // Figure out which outputs we still need (non-extract commands + cache misses)
                // Question: If there are no extracts/prints left, but there are some RunSchedule
                // commands, do we still run them to make sure the Vec<CommandOutput> exactly
                // matches the non-cache version? Or are we okay to drop those since we probably
                // don't care about the RunReport absent any printing/extracting
                // If there are none, we're done-- don't even need to make an egraph
                // Else, make an egraph and run all commands except cache hitting extracts (call run_program_with_reporter)
                // Splice the cache hit extract results into the Vec<CommandOutput> we get back at the right places
            } else {
                // Same logic as above, but without reporting overhead
            }
        }
        ServeMode::Batch {
            input_dir,
            output_dir,
        } => todo!("not yet implemented"),
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
