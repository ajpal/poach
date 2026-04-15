use egglog::{
    report::{MetricValue, Reporter},
    EGraph,
};
use std::{fs::File, path::PathBuf, process::exit};

use clap::{Args, Parser, Subcommand};
use walkdir::WalkDir;

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
    ///   writes output file to the output directory
    Single {
        input_file: PathBuf,
        output_dir: PathBuf,    
    },

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
    rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build_global()
                .unwrap();

    match arg.serve_command {
        None => {
            let mut egraph = EGraph::default();

            match egraph.repl(egglog::RunMode::Normal) {
                Ok(_) => {}
                _ => {
                    exit(-1);
                }
            }
        }
        Some(cmd) => {
            match cmd {
                ServeCommands::Single { input_file: input, output_dir: out_dir } => {
                    let mut egraph = EGraph::default();
                    let mut reporter = Reporter::new();
                    std::fs::create_dir_all(&out_dir).expect("Failed to create output dir");

                    let serve_timer = reporter.start_timer("serve".to_string());
                    let read_timer = reporter.start_timer("read_program".to_string());
                    let program = std::fs::read_to_string(input.as_path()).unwrap_or_else(|_| {
                        let arg = input.to_string_lossy();
                        panic!("Failed to read file {arg}")
                    });
                    reporter.finish_timer(read_timer);

                    let run_timer = reporter.start_timer("run_program".to_string());
                    let parsed = egraph
                        .parser
                        .get_program_from_string(Some(input.to_str().unwrap().into()), &program)
                        .unwrap_or_else(|err| panic!("Failed to parse {}: {err}", input.display()));
                    match egraph.run_program_with_reporter(parsed, &mut reporter) {
                        Ok(msgs) => {
                            reporter.finish_timer(run_timer);
                            let num_outputs = msgs.len() as u64;
                            for msg in msgs {
                                print!("{msg}");
                            }
                            reporter.finish_timer(serve_timer);
                            reporter.record_size(
                                "program_bytes".to_string(),
                                MetricValue::Bytes(program.len() as u64),
                            );
                            reporter.record_size(
                                "num_tuples".to_string(),
                                MetricValue::Count(egraph.num_tuples() as u64),
                            );
                            reporter.record_size(
                                "num_outputs".to_string(),
                                MetricValue::Count(num_outputs),
                            );
                            let report_path = out_dir.join(
                                input
                                    .file_stem()
                                    .expect("input file should have a file stem"),
                            )
                            .with_extension("report.json");
                            let report_file = File::create(&report_path).expect("failed to create report file");
                            serde_json::to_writer_pretty(
                                report_file,
                                &reporter.build_report(input.to_string_lossy().into_owned()),
                            )
                            .expect("failed to write report");
                        }
                        _ => {
                            exit(-1);
                        }
                    }
                }
                ServeCommands::Batch {
                    input_dir,
                    output_dir,
                } => {
                    std::fs::create_dir_all(&output_dir).expect("Failed to create output dir");

                    // input_dir must be a directory for batch mode
                    if !input_dir.is_dir() {
                        panic!("Input path is not a directory: {:?}", input_dir);
                    }

                    let mut input_files: Vec<PathBuf> = WalkDir::new(&input_dir)
                        .into_iter()
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| entry.file_type().is_file())
                        .filter(|entry| {
                            entry.path().extension().and_then(|s| s.to_str()) == Some("egg")
                        })
                        .map(|entry| entry.path().to_path_buf())
                        .collect();
                    input_files.sort();
                    
                    for input in input_files {
                        println!("Processing {}", input.display());
                        let mut egraph = EGraph::default();

                        let program = std::fs::read_to_string(&input).expect("failed to read .egg file");

                        match egraph.parse_and_run_program(
                            Some(input.to_string_lossy().into_owned()),
                            &program,
                        ) {
                            Ok(msgs) => {
                                for msg in msgs {
                                    print!("{msg}");
                                }
                            }
                            Err(err) => {
                                eprintln!("Failed to process {}: {err}", input.display());
                                exit(-1);
                            }
                        }
                    }
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
