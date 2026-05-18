use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use hashbrown::HashMap;
use poach::Term;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize)]
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
fn parse_cached_term(td: &mut TermDag, s: &str) -> TermId {
    let expr = EgglogParser::default()
        .get_expr_from_string(None, s)
        .expect("failed to parse cached term");
    td.expr_to_term(&expr)
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

// Todo: incremental instead of overwrite
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
                } else {
                    panic!("Not a MultiExtract output")
                }
            }
            _ => panic!("Not an extract command"),
        }
    }
    cache
}

fn try_cache(cmd: &Command, cache: &TermCache) -> Option<CommandOutput> {
    let mut td = TermDag::default();
    match cmd {
        Command::Extract(_span, expr, variants_e) => {
            // Always a cache miss if `variants_e` is not a literal
            let n = variants_count(variants_e)?;

            if let Some(CacheEntry {
                terms: extracted_variants,
                lowest_cost,
                exhausted,
            }) = cache.get(&expr.to_string())
            {
                if n == 0 {
                    // Best
                    let term_id = parse_cached_term(&mut td, extracted_variants.first()?);
                    Some(CommandOutput::ExtractBest(td, *lowest_cost, term_id))
                } else if n <= extracted_variants.len() || *exhausted {
                    // Variants Hit
                    let n = n.min(extracted_variants.len());
                    let tids: Vec<TermId> = extracted_variants[..n]
                        .iter()
                        .map(|s| parse_cached_term(&mut td, s))
                        .collect();
                    Some(CommandOutput::ExtractVariants(td, tids))
                } else {
                    // Variants Miss (Not enough variants in cache)
                    None
                }
            } else {
                None
            }
        }
        Command::MultiExtract(_span, variants_e, exprs) => {
            // Always a cache miss if `variants_e` is not a literal
            let n = variants_count(variants_e)?;

            let mut all_tids = vec![];

            for expr in exprs {
                if let Some(CacheEntry {
                    terms: extracted_variants,
                    lowest_cost: _lowest_cost,
                    exhausted,
                }) = cache.get(&expr.to_string())
                {
                    if n > extracted_variants.len() && !*exhausted {
                        // Cache miss (Not enough variants)
                        return None;
                    }
                    let n = n.min(extracted_variants.len());
                    let tids: Vec<TermId> = extracted_variants[..n]
                        .iter()
                        .map(|s| parse_cached_term(&mut td, s))
                        .collect();
                    all_tids.push(tids);
                } else {
                    // Cache miss (No entry for expr)
                    return None;
                }
            }

            Some(CommandOutput::MultiExtractVariants(td, all_tids))
        }
        _ => panic!("Not an extract command"),
    }
}

fn serve(arg: ServeArgs) {
    match arg.mode {
        ServeMode::Streaming => todo!("not yet implemented"),
        ServeMode::Single { input_file } => {
            if arg.debug {
                let mut reporter = Reporter::new();

                let deserialize_timer =
                    reporter.new_timer("deserialize".to_string(), vec!["deserialize".to_string()]);
                let cache: TermCache = serde_json::from_reader(
                    std::fs::File::open(&arg.model_file).expect("open model file"),
                )
                .expect("deserialize model");
                reporter.record_timer(deserialize_timer);

                let input = std::fs::read_to_string(&input_file).expect("read input file");
                let mut egraph = EGraph::default();
                let program = egraph
                    .parser
                    .get_program_from_string(
                        Some(input_file.to_string_lossy().into_owned()),
                        &input,
                    )
                    .expect("parse input file");

                let cache_overhead_timer =
                    reporter.new_timer("cache_overhead".to_string(), vec!["overhead".to_string()]);
                let cmds_with_outputs_and_cache_results: Vec<_> = program
                    .iter()
                    .filter(|c| {
                        matches!(
                            c,
                            Command::Extract(..)
                                | Command::MultiExtract(..)
                                | Command::PrintOverallStatistics(..)
                                | Command::PrintFunction(..)
                                | Command::PrintSize(..)
                                | Command::UserDefined(..)
                                | Command::RunSchedule(..)
                        )
                    })
                    .map(|c| match c {
                        Command::Extract(..) | Command::MultiExtract(..) => {
                            (c, try_cache(c, &cache))
                        }
                        _ => (c, None),
                    })
                    .collect();

                // Count hits/misses
                let (cache_hits, cache_misses) = cmds_with_outputs_and_cache_results
                    .iter()
                    .filter(|(c, _)| matches!(c, Command::Extract(..) | Command::MultiExtract(..)))
                    .fold(
                        (0, 0),
                        |(h, m), (_, r)| {
                            if r.is_some() { (h + 1, m) } else { (h, m + 1) }
                        },
                    );
                reporter.record_size("cache_hits".to_string(), MetricValue::Count(cache_hits));
                reporter.record_size("cache_misses".to_string(), MetricValue::Count(cache_misses));

                // Figure out which outputs we still need
                let needs_egraph = cmds_with_outputs_and_cache_results
                    .iter()
                    .filter(|(cmd, cache_res)| match (cmd, cache_res) {
                        // Cache hit — we already have the answer.
                        (_, Some(_)) => false,
                        // Side-effect-only commands produce only run reports,
                        // which we're fine to skip when nothing else needs the egraph.
                        (Command::RunSchedule(..), _) => false,
                        (Command::UserDefined(_, name, _), _) if name == "run-schedule" => false,
                        // Otherwise we need the egraph.
                        _ => true,
                    })
                    .next()
                    .is_some();

                let all_outputs: Vec<_> = if needs_egraph {
                    // We need to build an egraph and run the program to get outputs that
                    // are not available from the Cache (either extraction cache misses or non-extraction outputs)

                    // Filter cache-hitting extractions out of the program
                    let egraph_program: Vec<Command> = program
                        .iter()
                        .filter(|c| match c {
                            Command::Extract(..) | Command::MultiExtract(..) => {
                                try_cache(c, &cache).is_none()
                            }
                            _ => true,
                        })
                        .cloned()
                        .collect();

                    reporter.record_timer(cache_overhead_timer);

                    let egraph_outputs = egraph
                        .run_program_with_reporter(egraph_program, &mut reporter)
                        .expect("run input file");

                    reporter.record_size(
                        "egraph_size".to_string(),
                        MetricValue::Count(egraph.num_tuples() as u64),
                    );

                    let mut egraph_iter = egraph_outputs.into_iter();
                    cmds_with_outputs_and_cache_results
                        .into_iter()
                        .map(|(_, cached)| {
                            if let Some(cached_output) = cached {
                                cached_output
                            } else {
                                egraph_iter
                                    .next()
                                    .expect("egraph produced fewer outputs than expected")
                            }
                        })
                        .collect()
                } else {
                    reporter.record_timer(cache_overhead_timer);
                    // There are no cache misses or non-extraction outputs, so we don't need an egraph at all

                    // No egraph
                    reporter.record_size("egraph_size".to_string(), MetricValue::Count(0));

                    cmds_with_outputs_and_cache_results
                        .into_iter()
                        // Skip running the egraph entirely, so skip run report outputs
                        .filter(|(cmd, _)| {
                            !matches!(cmd, Command::RunSchedule(..))
                                && !matches!(cmd, Command::UserDefined(_, name, _) if name == "run-schedule")
                        })
                        .map(|(_, cached)| cached.expect("output_needed empty implies all cached"))
                        .collect()
                };

                for msg in all_outputs {
                    println!("{}", msg);
                }

                serde_json::to_writer(&mut stderr(), &reporter.build_report())
                    .expect("Failed to serialize report");
            } else {
                // TODO: Figure out what to factor out of the above to reduce code duplication
                todo!("not done yet");
            }
        }
        ServeMode::Batch { .. } => todo!("not yet implemented"),
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
