use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use hashbrown::HashMap;
use poach::Term;
use serde::Serialize;

use ::poach::ast::{Command, Expr, Literal, Parser as EgglogParser};
use ::poach::report::{MetricValue, Reporter};
use ::poach::{CommandOutput, EGraph, TermDag, TermId};
use std::io::stderr;

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

#[derive(Serialize)]
struct CacheEntry {
    terms: Vec<String>,
    lowest_cost: u64,
    exhausted: bool,
}
type TermCache = HashMap<String, CacheEntry>;

/// Extract the integer count from a variants expression, if it is a literal.
/// Non-literal variant counts are skipped
fn variants_count(e: &Expr) -> Option<usize> {
    match e {
        Expr::Lit(_, Literal::Int(n)) if *n >= 0 => Some(*n as usize),
        _ => None,
    }
}

// TODO: Add cost model from the e-graph, this is just the default (term size)
fn term_cost(td: &TermDag, t: TermId) -> u64 {
    match td.get(t) {
        Term::App(_, children) => 1 + children.iter().map(|&c| term_cost(td, c)).sum::<u64>(),
        Term::Lit(_) | Term::Var(_) => 1,
    }
}

/// Re-parse a cached term string into a fresh `TermDag` rooted at the returned `TermId`.
/// Uses `Parser::default()`, which does not know about any user-defined sorts or constructors;
/// for current cache use this is fine because the cached strings are flat term shapes
/// (apps + literals). Revisit if user-defined macros start appearing in extracted terms.
fn parse_cached_term(td: &mut TermDag, s: &str) -> Option<TermId> {
    let expr = EgglogParser::default().get_expr_from_string(None, s).ok()?;
    Some(td.expr_to_term(&expr))
}

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
        // 1. Setup
        let mut egraph = EGraph::default();

        let input = std::fs::read_to_string(&arg.training_set).expect("read training set");
        let program = egraph
            .parser
            .get_program_from_string(
                Some(arg.training_set.to_string_lossy().into_owned()),
                &input,
            )
            .expect("parse training set");

        // 2. Run program
        let mut reporter = Reporter::new();
        let outputs = egraph
            .run_program_with_reporter(program.clone(), &mut reporter)
            .expect("run training set");

        // 3. Build extraction cache
        let extract_cmds: Vec<_> = program
            .iter()
            .filter(|c| matches!(c, Command::Extract(..) | Command::MultiExtract(..)))
            .collect();
        let extract_outs: Vec<_> = outputs
            .iter()
            .filter(|o| {
                matches!(
                    o,
                    CommandOutput::ExtractBest(..)
                        | CommandOutput::ExtractVariants(..)
                        | CommandOutput::MultiExtractVariants(..)
                )
            })
            .collect();
        assert!(extract_cmds.len() == extract_outs.len());

        let build_cache_timer =
            reporter.new_timer("build_model".to_string(), vec!["build_model".to_string()]);
        let cache = build_cache(extract_cmds, extract_outs);
        reporter.record_timer(build_cache_timer);

        // 4. Serialize extraction cache
        let serialize_timer =
            reporter.new_timer("serialize".to_string(), vec!["serialize".to_string()]);
        let model_file = std::fs::File::create(&arg.output_model_file).expect("create model file");
        serde_json::to_writer(model_file, &cache).expect("serialize and write model");
        reporter.record_timer(serialize_timer);

        let model_bytes = std::fs::metadata(&arg.output_model_file)
            .map(|m| m.len())
            .unwrap_or(0);

        reporter.record_size(
            "cache_keys".to_string(),
            MetricValue::Count(cache.len() as u64),
        );

        for (k, v) in cache {
            reporter.record_size(
                format!("variants[{k}]"),
                MetricValue::Count(v.terms.len() as u64),
            );
        }
        reporter.record_size(
            "egraph_num_tuples".to_string(),
            MetricValue::Count(egraph.num_tuples() as u64),
        );
        reporter.record_size(
            "serialized_model".to_string(),
            MetricValue::Bytes(model_bytes),
        );

        serde_json::to_writer(&mut stderr(), &reporter.build_report())
            .expect("Failed to serialize report");
    } else {
        let mut egraph = EGraph::default();

        let input = std::fs::read_to_string(&arg.training_set).expect("read training set");
        let program = egraph
            .parser
            .get_program_from_string(
                Some(arg.training_set.to_string_lossy().into_owned()),
                &input,
            )
            .expect("parse training set");

        let outputs = egraph
            .run_program(program.clone())
            .expect("run training set");

        let extract_cmds: Vec<_> = program
            .iter()
            .filter(|c| matches!(c, Command::Extract(..) | Command::MultiExtract(..)))
            .collect();
        let extract_outs: Vec<_> = outputs
            .iter()
            .filter(|o| {
                matches!(
                    o,
                    CommandOutput::ExtractBest(..)
                        | CommandOutput::ExtractVariants(..)
                        | CommandOutput::MultiExtractVariants(..)
                )
            })
            .collect();
        assert!(extract_cmds.len() == extract_outs.len());

        // Traverse extract commands/outputs pairwise and construct caches
        let cache = build_cache(extract_cmds, extract_outs);

        let model_file = std::fs::File::create(&arg.output_model_file).expect("create model file");
        serde_json::to_writer(model_file, &cache).expect("serialize and write model");
    }
}

fn build_cache(cmds: Vec<&Command>, outs: Vec<&CommandOutput>) -> TermCache {
    let mut cache = HashMap::default();
    for (cmd, out) in cmds.iter().zip(outs) {
        match cmd {
            Command::Extract(_span, expr, variants_e) => match out {
                CommandOutput::ExtractBest(term_dag, cost, term) => {
                    cache.insert(
                        expr.to_string(),
                        CacheEntry {
                            terms: vec![term_dag.to_string(*term)],
                            lowest_cost: *cost,
                            exhausted: false,
                        },
                    );
                }
                CommandOutput::ExtractVariants(term_dag, terms) => {
                    let variants: Vec<_> = terms.iter().map(|t| term_dag.to_string(*t)).collect();
                    let lowest_cost = terms
                        .first()
                        .map(|t| term_cost(term_dag, *t))
                        .expect("Empty variants");
                    let exhausted = variants_count(variants_e).is_some_and(|n| variants.len() < n);
                    cache.insert(
                        expr.to_string(),
                        CacheEntry {
                            terms: variants,
                            lowest_cost,
                            exhausted,
                        },
                    );
                }
                _ => panic!("Not an extract output"),
            },
            Command::MultiExtract(_span, variants_e, exprs) => {
                if let CommandOutput::MultiExtractVariants(term_dag, all_terms) = out {
                    let n = variants_count(variants_e);
                    for (expr, terms) in exprs.iter().zip(all_terms.iter()) {
                        let variants: Vec<_> =
                            terms.iter().map(|t| term_dag.to_string(*t)).collect();
                        let lowest_cost = terms
                            .first()
                            .map(|t| term_cost(term_dag, *t))
                            .expect("Empty variants");
                        let exhausted = n.is_some_and(|n| variants.len() < n);
                        cache.insert(
                            expr.to_string(),
                            CacheEntry {
                                terms: variants,
                                lowest_cost,
                                exhausted,
                            },
                        );
                    }
                }
            }
            _ => panic!("Not an extract command"),
        }
    }
    cache
}

fn serve(arg: ServeArgs) {
    match arg.mode {
        ServeMode::Streaming => todo!("not yet implemented"),
        ServeMode::Single { input_file } => {
            if arg.debug {
            } else {
            }
        }
        ServeMode::Batch { .. } => todo!("not yet implemented"),
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
