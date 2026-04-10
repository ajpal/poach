use poach::EGraph;

use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
    process::exit,
    time::Instant,
};

use clap::{Args, Parser, Subcommand};

use crate::report::{MetricValue, RunReport, with_report};

#[derive(Debug, Parser)]
#[command(version, about)]
#[command(propagate_version = true)]
struct Cli {
    /// Directory where run reports are written as JSON
    #[arg(short, long)]
    output_dir: PathBuf,

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
    let output_dir = cli.output_dir;
    let report_path = output_dir.join("report.json");
    let report = match cli.command {
        Commands::Train(arg) => train(arg),
        Commands::Serve(arg) => serve(arg),
        Commands::FineTune(arg) => fine_tune(arg),
        Commands::Test(_) => todo!("test not yet implemented"),
    };

    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|err| {
            panic!(
                "Failed to create report directory {}: {err}",
                parent.display()
            )
        });
    }

    let report_file = File::create(&report_path).unwrap_or_else(|err| {
        panic!(
            "Failed to create report file {}: {err}",
            report_path.display()
        )
    });
    serde_json::to_writer_pretty(report_file, &report).unwrap_or_else(|err| {
        panic!(
            "Failed to write report file {}: {err}",
            report_path.display()
        )
    });
}

/// VanillaEgglog's model is just unit
/// Still, it would create an empty file
fn train(arg: TrainArgs) -> RunReport {
    let (_, report) = with_report("train", |_reporter| {
        let _ = File::create(arg.output_model_file.as_path());
    });
    report
}

/// VanillaEgglog
fn serve(arg: ServeArgs) -> RunReport {
    let (_, report) = with_report("serve", |reporter| match arg.serve_command {
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
        Some(cmd) => match cmd {
            ServeCommands::Single { input_file: input } => {
                let started_at = Instant::now();

                let program = reporter.time("read_input", || {
                    std::fs::read_to_string(input.as_path())
                        .map_err(|_| format!("Failed to read file {}", input.display()))
                });
                let program = match program {
                    Ok(program) => program,
                    Err(err) => {
                        eprintln!("{err}");
                        exit(-1);
                    }
                };
                reporter.record_size("input_bytes", MetricValue::Bytes(program.len() as u64));

                let mut egraph = reporter.time("build_egraph", || {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(1)
                        .build_global()
                        .unwrap();
                    EGraph::default()
                });

                let outputs = reporter.time("run_program", || {
                    egraph.parse_and_run_program(Some(input.display().to_string()), &program)
                });
                let outputs = match outputs {
                    Ok(outputs) => outputs,
                    Err(err) => {
                        eprintln!("{err}");
                        exit(-1);
                    }
                };

                reporter.record_size("command_outputs", MetricValue::Count(outputs.len() as u64));
                reporter.record_size(
                    "egraph_tuples",
                    MetricValue::Count(egraph.num_tuples() as u64),
                );

                reporter.time("print_outputs", || {
                    let mut stdout = io::stdout();
                    for output in outputs {
                        write!(stdout, "{output}").expect("failed to write command output");
                    }
                });

                reporter.record_timing("serve", started_at.elapsed());
            }
            ServeCommands::Batch {
                input_dir: _,
                output_dir: _,
            } => {
                //TODO
                panic!("Batch not implemented");
            }
        },
    });
    report
}

/// VanillaEgglog's model is just unit
/// Still, it would create an empty file
fn fine_tune(arg: FineTuneArgs) -> RunReport {
    let (_, report) = with_report("fine_tune", |_reporter| {
        let _ = File::create(arg.output_model_file.as_path());
    });
    report
}
