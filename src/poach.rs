use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use hashbrown::HashMap;
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

#[derive(Debug, Default, Serialize, Deserialize)]
struct Model {
    best: HashMap<String, BestEntry>,
    variants: HashMap<String, VariantsEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BestEntry {
    term: String,
    cost: u64, // Need to cache what the cost was so we can reconstruct a CommandOutput on hit
}

#[derive(Debug, Serialize, Deserialize)]
struct VariantsEntry {
    variants: Vec<String>,
    // True when the extraction returned fewer variants than requested,
    // meaning this is all of the variants represented in the egraph
    // When true, we will return the cached result even when it's less
    // than the number of variants requested.
    exhausted: bool,
}

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

/// Extract the integer count from a variants expression, if it is a literal.
/// Non-literal variant counts are skipped both for training and for cache lookup.
fn variants_count(e: &Expr) -> Option<usize> {
    match e {
        Expr::Lit(_, Literal::Int(n)) if *n >= 0 => Some(*n as usize),
        _ => None,
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
        let mut egraph = EGraph::default();

        let input = std::fs::read_to_string(&arg.training_set).expect("read training set");
        let program = egraph
            .parser
            .get_program_from_string(
                Some(arg.training_set.to_string_lossy().into_owned()),
                &input,
            )
            .expect("parse training set");

        let mut reporter = Reporter::new();
        let outputs = egraph
            .run_program_with_reporter(program.clone(), &mut reporter)
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
        let build_cache_timer =
            reporter.new_timer("build_model".to_string(), vec!["build_model".to_string()]);
        let mut model = Model::default();
        for (cmd, out) in extract_cmds.iter().zip(extract_outs) {
            add_to_cache(&mut model, cmd, out);
        }
        reporter.record_timer(build_cache_timer);

        let serialize_timer =
            reporter.new_timer("serialize".to_string(), vec!["serialize".to_string()]);
        let model_file = std::fs::File::create(&arg.output_model_file).expect("create model file");
        serde_json::to_writer(model_file, &model).expect("serialize and write model");
        reporter.record_timer(serialize_timer);

        let model_bytes = std::fs::metadata(&arg.output_model_file)
            .map(|m| m.len())
            .unwrap_or(0);

        reporter.record_size(
            "best_cache_keys".to_string(),
            MetricValue::Count(model.best.len() as u64),
        );
        reporter.record_size(
            "variants_cache_keys".to_string(),
            MetricValue::Count(model.variants.len() as u64),
        );
        for (k, v) in &model.variants {
            reporter.record_size(
                format!("variants[{k}]"),
                MetricValue::Count(v.variants.len() as u64),
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
        let mut model = Model::default();
        for (cmd, out) in extract_cmds.iter().zip(extract_outs) {
            add_to_cache(&mut model, cmd, out);
        }

        let model_file = std::fs::File::create(&arg.output_model_file).expect("create model file");
        serde_json::to_writer(model_file, &model).expect("serialize and write model");
    }
}

// Add one extract command + output pair to the model
// ExtractBest populates the Best Cache
// ExtractVariants populates the Variants Cache
// MultiExtractVariants is decomposed into individual extractions, which
// each populate the Variants Cache
fn add_to_cache(model: &mut Model, cmd: &Command, result: &CommandOutput) {
    let get_variants_count = |e: &Expr| -> Option<usize> {
        if let Expr::Lit(_, Literal::Int(n)) = e {
            if *n >= 0 { Some(*n as usize) } else { None }
        } else {
            None
        }
    };

    match cmd {
        Command::Extract(_span, expr, variants_e) => match result {
            CommandOutput::ExtractBest(term_dag, cost, term) => {
                model.best.insert(
                    expr.to_string(),
                    BestEntry {
                        term: term_dag.to_string(*term),
                        cost: *cost,
                    },
                );
            }
            CommandOutput::ExtractVariants(term_dag, terms) => {
                let variants = terms
                    .iter()
                    .map(|t| term_dag.to_string(*t))
                    .collect::<Vec<_>>();

                let exhausted = get_variants_count(variants_e).is_some_and(|n| variants.len() < n);

                model.variants.insert(
                    expr.to_string(),
                    VariantsEntry {
                        variants,
                        exhausted,
                    },
                );
            }
            _ => panic!("Unexpected result to add to cache: {}", result),
        },
        Command::MultiExtract(_span, variants_e, exprs) => {
            if let CommandOutput::MultiExtractVariants(term_dag, all_terms) = result {
                let n = get_variants_count(variants_e);
                for (expr, terms) in exprs.iter().zip(all_terms.iter()) {
                    let variants = terms
                        .iter()
                        .map(|t| term_dag.to_string(*t))
                        .collect::<Vec<_>>();
                    let exhausted = n.is_some_and(|n| variants.len() < n);
                    model.variants.insert(
                        expr.to_string(),
                        VariantsEntry {
                            variants,
                            exhausted,
                        },
                    );
                }
            } else {
                panic!("Unexpected result to add to cache: {}", result)
            }
        }
        _ => panic!("Unexpected cmd to add to cache: {}", cmd),
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

                let mut reporter = Reporter::new();

                // Deserialize model into two caches: `best` and `variants`
                let deserialize_timer =
                    reporter.new_timer("deserialize".to_string(), vec!["deserialize".to_string()]);
                let model: Model = serde_json::from_reader(
                    std::fs::File::open(&arg.model_file).expect("open model file"),
                )
                .expect("deserialize model");
                reporter.record_timer(deserialize_timer);

                // read and parse program
                let input = std::fs::read_to_string(&input_file).expect("read input file");
                let mut egraph = EGraph::default();
                let program = egraph
                    .parser
                    .get_program_from_string(
                        Some(input_file.to_string_lossy().into_owned()),
                        &input,
                    )
                    .expect("parse input file");

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
                let (sources, egraph_program) = plan_serve(program, &model);

                // Question: If there are no extracts/prints left, but there are some RunSchedule
                // commands, do we still run them to make sure the Vec<CommandOutput> exactly
                // matches the non-cache version? Or are we okay to drop those since we probably
                // don't care about the RunReport absent any printing/extracting
                // If there are none, we're done-- don't even need to make an egraph
                let needs_egraph = sources
                    .iter()
                    .any(|s| matches!(s, OutputSource::FromEgraph));

                // Else, make an egraph and run all commands except cache hitting extracts (call run_program_with_reporter)
                let egraph_outputs = if needs_egraph {
                    egraph
                        .run_program_with_reporter(egraph_program, &mut reporter)
                        .expect("run input file")
                } else {
                    Vec::new()
                };

                // Splice the cache hit extract results into the Vec<CommandOutput> we get back at the right places
                let mut egraph_iter = egraph_outputs.into_iter();
                use std::io::Write as _;
                let mut out_handle = std::io::stdout();
                for source in sources {
                    let out = match source {
                        OutputSource::Cache(o) => o,
                        OutputSource::FromEgraph => egraph_iter
                            .next()
                            .expect("egraph produced fewer outputs than expected"),
                    };
                    write!(out_handle, "{out}").expect("write stdout");
                }

                reporter.record_size(
                    "egraph_num_tuples".to_string(),
                    MetricValue::Count(if needs_egraph {
                        egraph.num_tuples() as u64
                    } else {
                        0
                    }),
                );

                let mut err = std::io::stderr();
                let report_json =
                    serde_json::to_string(&reporter.build_report()).expect("serialize report");
                writeln!(err, "{report_json}").ok();
            } else {
                // Same logic as above, but without reporting overhead
                let model_bytes = std::fs::read(&arg.model_file).expect("read model file");
                let model: Model = serde_json::from_slice(&model_bytes).expect("deserialize model");
                let input = std::fs::read_to_string(&input_file).expect("read input file");
                let mut egraph = EGraph::default();
                let program = egraph
                    .parser
                    .get_program_from_string(
                        Some(input_file.to_string_lossy().into_owned()),
                        &input,
                    )
                    .expect("parse input file");
                let (sources, egraph_program) = plan_serve(program, &model);
                let needs_egraph = sources
                    .iter()
                    .any(|s| matches!(s, OutputSource::FromEgraph));
                let egraph_outputs = if needs_egraph {
                    egraph.run_program(egraph_program).expect("run input file")
                } else {
                    Vec::new()
                };
                let mut egraph_iter = egraph_outputs.into_iter();
                use std::io::Write as _;
                let mut out_handle = std::io::stdout();
                for source in sources {
                    let out = match source {
                        OutputSource::Cache(o) => o,
                        OutputSource::FromEgraph => egraph_iter
                            .next()
                            .expect("egraph produced fewer outputs than expected"),
                    };
                    write!(out_handle, "{out}").expect("write stdout");
                }
            }
        }
        ServeMode::Batch { .. } => todo!("not yet implemented"),
    }
}

/// Where a slot in the final output stream comes from.
#[allow(clippy::large_enum_variant)]
enum OutputSource {
    Cache(CommandOutput),
    FromEgraph,
}

/// Walk the parsed program once. For each command:
/// - If it is an Extract/MultiExtract that we can serve from the cache, record a `Cache` source
///   and drop the command from what we send to the egraph.
/// - Otherwise, record a `FromEgraph` source for output-producing commands and forward the
///   command to the egraph (so any side effects still happen).
fn plan_serve(program: Vec<Command>, model: &Model) -> (Vec<OutputSource>, Vec<Command>) {
    let mut sources = Vec::new();
    let mut egraph_program = Vec::with_capacity(program.len());
    for cmd in program {
        if let Some(out) = try_cache_hit(&cmd, model) {
            sources.push(OutputSource::Cache(out));
        } else {
            if produces_output(&cmd) {
                sources.push(OutputSource::FromEgraph);
            }
            egraph_program.push(cmd);
        }
    }
    (sources, egraph_program)
}

/// Commands whose execution yields a `CommandOutput` in `run_program`'s return value.
fn produces_output(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::RunSchedule(..)
            | Command::PrintOverallStatistics(..)
            | Command::Extract(..)
            | Command::MultiExtract(..)
            | Command::PrintFunction(..)
            | Command::PrintSize(..)
            | Command::UserDefined(..)
    )
}

/// Attempt to satisfy an extract command from the cache.
/// Returns `None` (cache miss / not an extract / non-literal variant count), and the command
/// is then forwarded to the egraph.
fn try_cache_hit(cmd: &Command, model: &Model) -> Option<CommandOutput> {
    match cmd {
        Command::Extract(_, expr, variants_e) => {
            let n = variants_count(variants_e)?;
            let key = expr.to_string();
            if n == 0 {
                let entry = model.best.get(&key)?;
                let mut td = TermDag::default();
                let tid = parse_cached_term(&mut td, &entry.term)?;
                Some(CommandOutput::ExtractBest(td, entry.cost, tid))
            } else {
                if let Some(entry) = model.variants.get(&key) {
                    if n <= entry.variants.len() || entry.exhausted {
                        let take = n.min(entry.variants.len());
                        let mut td = TermDag::default();
                        let mut tids = Vec::with_capacity(take);
                        for s in &entry.variants[..take] {
                            tids.push(parse_cached_term(&mut td, s)?);
                        }
                        return Some(CommandOutput::ExtractVariants(td, tids));
                    }
                }
                // Variants miss. Per the resolved Question on `extract-variants 1`:
                // fall back to the `best` cache when exactly one variant was requested.
                if n == 1 {
                    let entry = model.best.get(&key)?;
                    let mut td = TermDag::default();
                    let tid = parse_cached_term(&mut td, &entry.term)?;
                    Some(CommandOutput::ExtractVariants(td, vec![tid]))
                } else {
                    None
                }
            }
        }
        Command::MultiExtract(_, variants_e, exprs) => {
            let n = variants_count(variants_e)?;
            let mut td = TermDag::default();
            let mut all_tids: Vec<Vec<TermId>> = Vec::with_capacity(exprs.len());
            for expr in exprs {
                let key = expr.to_string();
                let entry = model.variants.get(&key)?;
                if !(n <= entry.variants.len() || entry.exhausted) {
                    return None;
                }
                let take = n.min(entry.variants.len());
                let mut tids = Vec::with_capacity(take);
                for s in &entry.variants[..take] {
                    tids.push(parse_cached_term(&mut td, s)?);
                }
                all_tids.push(tids);
            }
            Some(CommandOutput::MultiExtractVariants(td, all_tids))
        }
        _ => None,
    }
}

fn fine_tune(arg: FineTuneArgs) {
    println!("fine_tune({:?})", arg);
    //TODO
}
