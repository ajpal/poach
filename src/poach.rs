use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};

use poach::ast::GenericCommand;
use poach::report::{MetricValue, Reporter};
use poach::{EGraph, Value};

use std::fs::File;
use std::io::{prelude::*, stderr};
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
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();

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

fn serialize_model_to_file(model: &Model, output_file: &Path) -> usize {
    let mut buf = FlexbufferSerializer::new();
    Serialize::serialize(model, &mut buf).expect("Failed to serialize the model to Flexbuffer");
    let serialized_size = buf.view().len();

    let Ok(mut file) = File::create(output_file) else {
        panic!("Failed to create file");
    };
    let _ = file.write_all(buf.view());
    serialized_size
}

fn train(arg: TrainArgs) {
    let mut egraph = EGraph::default();

    let input = arg.training_set;
    let filename = input.to_str().unwrap().to_string();

    if arg.debug {
        let mut reporter = Reporter::new();

        let read_timer = reporter.new_timer("read_input".to_string(), vec!["io".to_string()]);
        let program = std::fs::read_to_string(input.as_path())
            .unwrap_or_else(|_| panic!("Failed to read file {filename}"));
        reporter.record_timer(read_timer);

        let parsed = egraph
            .parser
            .get_program_from_string(Some(filename.clone()), &program)
            .unwrap_or_else(|e| panic!("Failed to parse {filename} with error {e:?}"));

        if let Err(e) = egraph.run_program_with_reporter(parsed, &mut reporter) {
            panic!("Failed to execute {filename} with error {e:?}");
        }

        let extract_timer = reporter.new_timer(
            "build_extraction_cache".to_string(),
            vec!["extraction".to_string()],
        );
        egraph.stabilize();
        let cache = egraph.build_extraction_cache();
        reporter.record_timer(extract_timer);

        let model = Model { egraph, cache };

        let serialize_timer =
            reporter.new_timer("serialize_model".to_string(), vec!["io".to_string()]);
        let serialized_size = serialize_model_to_file(&model, arg.output_model_file.as_path());
        reporter.record_timer(serialize_timer);
        reporter.record_size(
            "model_bytes".to_string(),
            MetricValue::Bytes(serialized_size as u64),
        );

        serde_json::to_writer(&mut stderr(), &reporter.build_report())
            .expect("Failed to serialize report");
        eprintln!();
    } else {
        let program = std::fs::read_to_string(input.as_path())
            .unwrap_or_else(|_| panic!("Failed to read file {filename}"));

        if let Err(e) = egraph.parse_and_run_program(Some(filename.clone()), &program) {
            panic!("Failed to execute {filename} with error {e:?}");
        }

        egraph.stabilize();
        let cache = egraph.build_extraction_cache();
        let model = Model { egraph, cache };
        let _ = serialize_model_to_file(&model, arg.output_model_file.as_path());
    }
}

fn deserialize_model_from_file(model_file: &Path) -> Model {
    let Ok(mut file) = File::open(model_file) else {
        panic!("Failed to open input model file");
    };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .expect("Failed to read from file");

    let root = Reader::get_root(buf.as_slice()).unwrap();
    let mut model = Model::deserialize(root).unwrap();
    model
        .egraph
        .restore_deserialized_runtime()
        .expect("Failed to restore deserialized runtime");

    model
}

/// Run the input commands one by one, intercepting `(extract e)` (variants == 0)
/// to short-circuit on a cache hit. When a `Reporter` is supplied, each step is
/// timed; when it isn't, the loop runs with no reporting overhead.
fn run_serve_commands(
    egraph: &mut EGraph,
    cache_map: &std::collections::HashMap<(String, Value), String>,
    commands: Vec<poach::ast::Command>,
    mut reporter: Option<&mut Reporter>,
) {
    for cmd in commands {
        if let GenericCommand::Extract(_, expr, variants) = &cmd {
            let n_val = egraph
                .eval_expr(variants)
                .expect("failed to evaluate variants count");
            let n: i64 = egraph.value_to_base(n_val.1);

            // n == 0 is egglog's "extract the single best term" case;
            // n > 0 asks for n variants, which the cache doesn't store.
            if n == 0 {
                let cache_lookup_timer = reporter.as_deref_mut().map(|r| {
                    r.new_timer("cache_lookup".to_string(), vec!["extraction".to_string()])
                });
                let (sort, value) = egraph
                    .eval_expr(expr)
                    .expect("failed to evaluate extract expr");
                let canon = egraph.get_canonical_value(value, &sort);
                let hit = cache_map.get(&(sort.name().to_owned(), canon));
                if let Some(cached) = hit {
                    println!("{cached}");
                }
                if let (Some(r), Some(h)) = (reporter.as_deref_mut(), cache_lookup_timer) {
                    r.record_timer(h);
                }
                if hit.is_some() {
                    continue;
                }
            }
        }
        let result = match reporter.as_deref_mut() {
            Some(r) => egraph.run_program_with_reporter(vec![cmd], r),
            None => egraph.run_program(vec![cmd]),
        };
        match result {
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

fn serve(arg: ServeArgs) {
    match arg.mode {
        ServeMode::Streaming => {
            // The extraction cache is dropped here: streaming mode forwards to
            // the egglog REPL, which doesn't expose a hook for command-by-command
            // interception.
            let mut egraph = deserialize_model_from_file(arg.model_file.as_path()).egraph;
            match egraph.repl(poach::RunMode::Normal) {
                Ok(_) => {}
                _ => {
                    exit(-1);
                }
            }
        }
        ServeMode::Single { input_file: input } => {
            let filename = input.to_str().unwrap().to_string();

            if arg.debug {
                let mut reporter = Reporter::new();

                let read_model_timer =
                    reporter.new_timer("read_model_file".to_string(), vec!["io".to_string()]);
                let mut file =
                    File::open(arg.model_file.as_path()).expect("Failed to open input model file");
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)
                    .expect("Failed to read from file");
                reporter.record_timer(read_model_timer);
                reporter.record_size(
                    "model_bytes".to_string(),
                    MetricValue::Bytes(buf.len() as u64),
                );

                let deserialize_timer =
                    reporter.new_timer("deserialize_model".to_string(), vec!["io".to_string()]);
                let root = Reader::get_root(buf.as_slice()).unwrap();
                let mut model = Model::deserialize(root).unwrap();
                model
                    .egraph
                    .restore_deserialized_runtime()
                    .expect("Failed to restore deserialized runtime");
                reporter.record_timer(deserialize_timer);
                let Model { mut egraph, cache } = model;

                let read_input_timer =
                    reporter.new_timer("read_input_program".to_string(), vec!["io".to_string()]);
                let program = std::fs::read_to_string(input.as_path())
                    .unwrap_or_else(|_| panic!("Failed to read file {filename}"));
                reporter.record_timer(read_input_timer);

                let cache_map: std::collections::HashMap<(String, Value), String> =
                    cache.into_iter().collect();

                let commands = egraph
                    .parser
                    .get_program_from_string(Some(filename.clone()), &program)
                    .unwrap_or_else(|e| panic!("Failed to parse {filename}: {e:?}"));

                run_serve_commands(&mut egraph, &cache_map, commands, Some(&mut reporter));

                serde_json::to_writer(&mut stderr(), &reporter.build_report())
                    .expect("Failed to serialize report");
                eprintln!();
            } else {
                let Model { mut egraph, cache } =
                    deserialize_model_from_file(arg.model_file.as_path());

                let program = std::fs::read_to_string(input.as_path())
                    .unwrap_or_else(|_| panic!("Failed to read file {filename}"));

                let cache_map: std::collections::HashMap<(String, Value), String> =
                    cache.into_iter().collect();

                let commands = egraph
                    .parser
                    .get_program_from_string(Some(filename.clone()), &program)
                    .unwrap_or_else(|e| panic!("Failed to parse {filename}: {e:?}"));

                run_serve_commands(&mut egraph, &cache_map, commands, None);
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
