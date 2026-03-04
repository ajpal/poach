use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use egglog::ast::{
    all_sexps, GenericAction, GenericCommand, GenericExpr, GenericFact, GenericRunConfig,
    GenericSchedule, Sexp, SexpParser,
};
use egglog::extract::DefaultCost;
use egglog::{CommandOutput, EGraph, TimedEgraph};
use env_logger::Env;
use hashbrown::HashMap;
use serde::Serialize;

use std::fmt::{Debug, Display};
use std::fs::{self, read_to_string, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Debug)]
enum RunMode {
    // For each egg file under the input path,
    //      run the egglog program and record timing information. Do not serialize.
    //      Save the complete timeline, for consumption by the nightly frontend.
    TimelineOnly,

    // For each egg file under the input path,
    //      run the egglog program and record timing information.
    //      Serialize to disk.
    //      Save the complete timeline, for consumption by the nightly frontend.
    Serialize,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph, recording timing information.
    //      Deserialize the serialized egraph, recording timing information.
    //      Assert the deserialized egraph has the same size as the initial egraph
    //      Save the complete timeline, for consumption by the nightly frontend.
    SequentialRoundTrip,

    // For each egg file under the input path,
    //      Run the egglog program.
    //      Round trip to file twice.
    //      Assert that the second round trip is idempotent (though the first may not be), crash if not.
    IdempotentRoundTrip,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph using both the poach serialization code and
    //      the visualizer serialization code, which serializes only the parent-child relationships
    //      Save the complete timeline, for consumption by the nightly frontend.
    OldSerialize,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Round trip to JSON Value, but do not read/write from file
    //      Assert the deserialized egraph has hthe same size as the initial egraph.
    //      Save the completed timeline, for consumption by the nightly frontend
    NoIO,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information
    //      Round trip to JSON Value, no File I/O
    //      Find all extract commands from the egglog program and perform
    //      the same extractions on the deserialized egraph
    //      Ensure the results are the same
    //      Save the completed timeline, for consumption by the nighly frontend
    Extract,

    // Requires initial-egraph to be provided via Args
    // For each egg file under the input path,
    //      Run the egglog program from a fresh egraph and record extract outputs.
    //      Deserialize the initial egraph.
    //      Run the egglog program, skipping declarations of Sorts and Rules.
    //      Compare extract outputs between the two runs.
    //      Save the completed timeline, for consumption by the nightly frontend
    Mine,
}

impl Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RunMode::TimelineOnly => "timeline",
                RunMode::SequentialRoundTrip => "sequential",
                RunMode::Serialize => "serialize",
                RunMode::IdempotentRoundTrip => "idempotent",
                RunMode::OldSerialize => "old-serialize",
                RunMode::NoIO => "no-io",
                RunMode::Extract => "extract",
                RunMode::Mine => "mine",
            }
        )
    }
}

#[derive(Debug, Parser)]
#[command(version = env!("FULL_VERSION"), about= env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    input_path: PathBuf,
    output_dir: PathBuf,
    run_mode: RunMode,

    // If this is a single file, it will be used as the initial egraph for
    // every file in the input_path directory
    // If it is a directory, we will look for a file matching the name of each
    // file in the input_path directory
    #[arg(long)]
    initial_egraph: Option<PathBuf>,
}

fn check_egraph_number(egraph: &TimedEgraph, expected: usize) -> Result<()> {
    if egraph.egraphs().len() != expected {
        anyhow::bail!(
            "Expected {} egraphs, found {}",
            expected,
            egraph.egraphs().len()
        );
    }
    Ok(())
}

fn check_egraph_size(egraph: &TimedEgraph) -> Result<()> {
    let expected = egraph.num_tuples();
    for eg in egraph.egraphs().iter() {
        if eg.num_tuples() != expected {
            anyhow::bail!("Expected {} tuples, found {}", expected, eg.num_tuples());
        }
    }
    Ok(())
}

fn check_idempotent(p1: &PathBuf, p2: &PathBuf, name: &str, out_dir: &PathBuf) {
    let json1: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(p1).expect(&format!("failed to open {}", p1.display())),
    )
    .expect(&format!("failed to parse {}", p1.display()));

    let json2: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(p2).expect(&format!("failed to open {}", p2.display())),
    )
    .expect(&format!("failed to parse {}", p2.display()));

    if let Some(diff) = serde_json_diff::values(json1, json2) {
        let file = fs::File::create(out_dir.join(format!("{name}-diff.json")))
            .expect("Failed to create diff file");
        serde_json::to_writer_pretty(BufWriter::new(file), &diff)
            .expect("failed to serialize diff");
        panic!("Diff for {}", name)
    }
}

fn benchmark_name(egg_file: &Path) -> &str {
    egg_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
}

fn process_files<F>(
    files: &[PathBuf],
    out_dir: &PathBuf,
    initial_egraph: Option<&Path>,
    mut f: F,
) -> (Vec<String>, Vec<(String, String)>)
where
    F: FnMut(&PathBuf, &PathBuf, &mut TimedEgraph) -> Result<()>,
{
    let mut failures = vec![];
    let mut successes = vec![];
    for (idx, file) in files.iter().enumerate() {
        let name = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let mut timed_egraph = if let Some(path) = initial_egraph {
            if path.is_file() {
                TimedEgraph::new_from_file(path)
            } else {
                TimedEgraph::new_from_file(&path.join(format!("{name}-serialize.json")))
            }
        } else {
            TimedEgraph::new()
        };

        match f(file, out_dir, &mut timed_egraph) {
            Ok(_) => {
                successes.push(name.to_string());
                println!("[{}/{}] {} : SUCCESS", idx + 1, files.len(), name)
            }
            Err(e) => {
                failures.push((name.to_string(), format!("{}", e)));
                println!("[{}/{}] {} : FAILURE {}", idx + 1, files.len(), name, e)
            }
        }
    }
    if failures.len() == 0 {
        println!("0 failures out of {} files", files.len());
    } else {
        println!("{} failures out of {} files", failures.len(), files.len());
        for (name, reason) in failures.iter() {
            println!("{} | {}", name, reason);
        }
    }
    (successes, failures)
}

fn collect_extract_outputs(outputs: Vec<CommandOutput>) -> Vec<CommandOutput> {
    outputs
        .into_iter()
        .filter(|output| {
            matches!(
                output,
                CommandOutput::ExtractBest(_, _, _)
                    | CommandOutput::ExtractVariants(_, _)
                    | CommandOutput::MultiExtractVariants(_, _)
            )
        })
        .collect()
}

#[derive(Serialize)]
struct MineExtractComparison {
    initial_term: String,
    initial_cost: DefaultCost,
    final_term: String,
    final_cost: DefaultCost,
}

fn extract_term_costs_from_output(outputs: &[CommandOutput]) -> Result<Vec<(String, DefaultCost)>> {
    let mut pairs = Vec::new();
    for output in outputs {
        match output {
            CommandOutput::ExtractBest(dag, cost, term) => {
                pairs.push((dag.to_string(*term), *cost));
            }
            CommandOutput::ExtractVariants(dag, variants) => {
                pairs.extend(
                    variants
                        .iter()
                        .map(|(cost, term)| (dag.to_string(*term), *cost)),
                );
            }
            CommandOutput::MultiExtractVariants(dag, groups) => {
                pairs.extend(groups.iter().flat_map(|group| {
                    group
                        .iter()
                        .map(|(cost, term)| (dag.to_string(*term), *cost))
                }));
            }
            _ => anyhow::bail!("Not an extract output: {output:?}"),
        }
    }
    Ok(pairs)
}

fn add_mine_extract_summary(
    report: &mut HashMap<String, Vec<MineExtractComparison>>,
    benchmark: &str,
    initial_extracts: &[CommandOutput],
    final_extracts: &[CommandOutput],
) -> Result<()> {
    let initial_pairs = extract_term_costs_from_output(initial_extracts)?;
    let final_pairs = extract_term_costs_from_output(final_extracts)?;
    if initial_pairs.len() != final_pairs.len() {
        anyhow::bail!(
            "extract lengths mismatch for {}: {} != {}",
            benchmark,
            initial_pairs.len(),
            final_pairs.len()
        );
    }

    report.insert(
        benchmark.to_string(),
        initial_pairs
            .into_iter()
            .zip(final_pairs)
            .map(
                |((initial_term, initial_cost), (final_term, final_cost))| {
                    MineExtractComparison {
                        initial_term,
                        initial_cost,
                        final_term,
                        final_cost,
                    }
                },
            )
            .collect(),
    );
    Ok(())
}

fn poach(
    files: Vec<PathBuf>,
    out_dir: &PathBuf,
    run_mode: RunMode,
    initial_egraph: Option<PathBuf>,
) -> (Vec<String>, Vec<(String, String)>) {
    match run_mode {
        RunMode::TimelineOnly => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir, timed_egraph| {
                let name = benchmark_name(egg_file);
                timed_egraph.run_from_file(egg_file)?;
                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;

                Ok(())
            },
        ),

        RunMode::Serialize => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir, timed_egraph| {
                let name = benchmark_name(egg_file);
                let outputs = timed_egraph.run_from_file(egg_file)?;
                for output in outputs {
                    if matches!(
                        output,
                        CommandOutput::ExtractBest(_, _, _)
                            | CommandOutput::ExtractVariants(_, _)
                            | CommandOutput::MultiExtractVariants(_, _)
                    ) {
                        print!("{output}");
                    }
                }
                timed_egraph.to_file(&out_dir.join(format!("{name}-serialize.json")))?;
                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;
                Ok(())
            },
        ),

        RunMode::SequentialRoundTrip => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir: &PathBuf, timed_egraph| {
                let name = benchmark_name(egg_file);
                timed_egraph.run_from_file(egg_file)?;
                let s1 = out_dir.join(format!("{name}-serialize1.json"));

                timed_egraph
                    .to_file(&s1)
                    .context("Failed to write s1.json")?;

                timed_egraph
                    .from_file(&s1)
                    .context("failed to read s1.json")?;

                check_egraph_number(&timed_egraph, 2)?;

                check_egraph_size(&timed_egraph)?;

                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;
                Ok(())
            },
        ),

        RunMode::IdempotentRoundTrip => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir, timed_egraph| {
                let name = benchmark_name(egg_file);
                timed_egraph.run_from_file(egg_file)?;
                let s1 = out_dir.join(format!("{name}-serialize1.json"));
                let s2 = out_dir.join(format!("{name}-serialize2.json"));
                let s3 = out_dir.join(format!("{name}-serialize3.json"));

                timed_egraph
                    .to_file(&s1)
                    .context("failed to serialize s1.json")?;

                timed_egraph
                    .from_file(&s1)
                    .context("failed to read s1.json")?;

                timed_egraph
                    .to_file(&s2)
                    .context("failed to serialize s2.json")?;

                timed_egraph
                    .from_file(&s2)
                    .context("failed to read s2.json")?;

                timed_egraph
                    .to_file(&s3)
                    .context("failed to serialize s3.json")?;

                timed_egraph
                    .from_file(&s3)
                    .context("failed to read s3.json")?;

                check_egraph_number(&timed_egraph, 4)?;
                check_egraph_size(&timed_egraph)?;
                check_idempotent(&s2, &s3, name, out_dir);

                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;
                Ok(())
            },
        ),

        RunMode::OldSerialize => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir, timed_egraph| {
                let name = benchmark_name(egg_file);
                timed_egraph.run_from_file(egg_file)?;

                timed_egraph
                    .to_file(&out_dir.join(format!("{name}-serialize-poach.json")))
                    .context("failed to write poach.json")?;

                timed_egraph
                    .old_serialize_egraph(&out_dir.join(format!("{name}-serialize-old.json")))
                    .context("Failed to serialize old.json")?;

                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;
                Ok(())
            },
        ),

        RunMode::NoIO => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir, timed_egraph| {
                let name = benchmark_name(egg_file);
                timed_egraph.run_from_file(egg_file)?;

                let value = timed_egraph
                    .to_value()
                    .context("Failed to encode egraph as json")?;

                timed_egraph
                    .from_value(value)
                    .context("failed to decode egraph from json")?;

                check_egraph_number(&timed_egraph, 2)?;

                check_egraph_size(&timed_egraph)?;

                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;

                Ok(())
            },
        ),

        RunMode::Extract => process_files(
            &files,
            out_dir,
            initial_egraph.as_deref(),
            |egg_file, out_dir, timed_egraph| {
                let name = benchmark_name(egg_file);
                let initial_extracts =
                    collect_extract_outputs(timed_egraph.run_from_file(egg_file)?);

                let program_string = &read_to_string(egg_file)?;

                let is_extract = |sexp: &&Sexp| {
                    if let Sexp::List(xs, _) = sexp {
                        if !xs.is_empty() {
                            match &xs[0] {
                                Sexp::Atom(s, _) => s == "extract",
                                _ => false,
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                let all_sexps = all_sexps(SexpParser::new(None, program_string))?;
                let extracts: String = all_sexps
                    .iter()
                    .filter(is_extract)
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                let extract_cmds = timed_egraph
                    .egraphs
                    .last_mut()
                    .expect("there are no egraphs")
                    .parser
                    .get_program_from_string(None, &extracts)?;

                let value = timed_egraph
                    .to_value()
                    .context("Failed to encode egraph as JSON")?;

                timed_egraph
                    .from_value(value)
                    .context("failed to decode egraph from json")?;

                check_egraph_number(&timed_egraph, 2)?;

                let final_extracts = collect_extract_outputs(
                    timed_egraph.run_program_with_timeline(extract_cmds, &extracts)?,
                );

                let initial_pairs = extract_term_costs_from_output(&initial_extracts)?;
                let final_pairs = extract_term_costs_from_output(&final_extracts)?;
                if initial_pairs != final_pairs {
                    anyhow::bail!(
                        "extract outputs differ:\ninitial: {:?}\nfinal: {:?}",
                        initial_pairs,
                        final_pairs
                    );
                }

                timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;

                Ok(())
            },
        ),

        RunMode::Mine => {
            assert!(
                initial_egraph.is_some(),
                "initial_egraph must be provided via CLI args for Mine run mode"
            );
            let mut mine_extract_report = HashMap::new();
            let result = process_files(
                &files,
                out_dir,
                initial_egraph.as_deref(),
                |egg_file, out_dir, timed_egraph| {
                    // First, run the file on a blank e-graph and track extracts
                    let mut fresh_egraph = TimedEgraph::new();
                    let fresh_extracts =
                        collect_extract_outputs(fresh_egraph.run_from_file(egg_file)?);

                    let name = benchmark_name(egg_file);
                    // Namespace to avoid shadowing
                    #[derive(Default)]
                    struct Namespace {
                        map: HashMap<String, String>,
                    }

                    impl Namespace {
                        fn add(&mut self, name: String) -> String {
                            if self.map.contains_key(&name) {
                                panic!("duplicate variable names")
                            } else {
                                let namespaced = format!("{name}@@");
                                self.map.insert(name.clone(), namespaced.clone());
                                namespaced
                            }
                        }

                        fn get(&self, name: String) -> String {
                            self.map.get(&name).unwrap_or(&name).to_string()
                        }

                        fn replace_expr(
                            &self,
                            expr: GenericExpr<String, String>,
                        ) -> GenericExpr<String, String> {
                            match expr {
                                GenericExpr::Var(span, n) => GenericExpr::Var(span, self.get(n)),
                                GenericExpr::Call(span, h, generic_exprs) => GenericExpr::Call(
                                    span,
                                    self.get(h),
                                    generic_exprs
                                        .into_iter()
                                        .map(|x| self.replace_expr(x))
                                        .collect(),
                                ),
                                GenericExpr::Lit(span, literal) => GenericExpr::Lit(span, literal),
                            }
                        }

                        fn replace_fact(
                            &self,
                            fact: GenericFact<String, String>,
                        ) -> GenericFact<String, String> {
                            match fact {
                                GenericFact::Eq(span, e1, e2) => GenericFact::Eq(
                                    span,
                                    self.replace_expr(e1),
                                    self.replace_expr(e2),
                                ),
                                GenericFact::Fact(e) => GenericFact::Fact(self.replace_expr(e)),
                            }
                        }

                        fn replace_sched(
                            &self,
                            schedule: GenericSchedule<String, String>,
                        ) -> GenericSchedule<String, String> {
                            match schedule {
                                GenericSchedule::Saturate(span, sched) => {
                                    GenericSchedule::Saturate(
                                        span,
                                        Box::new(self.replace_sched(*sched)),
                                    )
                                }
                                GenericSchedule::Repeat(span, n, sched) => GenericSchedule::Repeat(
                                    span,
                                    n,
                                    Box::new(self.replace_sched(*sched)),
                                ),
                                GenericSchedule::Run(span, config) => GenericSchedule::Run(
                                    span,
                                    GenericRunConfig {
                                        ruleset: config.ruleset,
                                        until: config.until.map(|facts| {
                                            facts
                                                .into_iter()
                                                .map(|f| self.replace_fact(f))
                                                .collect()
                                        }),
                                    },
                                ),
                                GenericSchedule::Sequence(span, scheds) => {
                                    GenericSchedule::Sequence(
                                        span,
                                        scheds.into_iter().map(|x| self.replace_sched(x)).collect(),
                                    )
                                }
                            }
                        }
                    }
                    let mut namespace = Namespace::default();

                    let program_string = &read_to_string(egg_file)?;

                    let all_sexps = all_sexps(SexpParser::new(None, program_string))?;

                    let all_cmds = EGraph::default()
                        .parser
                        .get_program_from_string(None, &program_string)?;

                    assert!(all_cmds.len() == all_sexps.len());

                    let (filtered_cmds, filtered_sexps): (Vec<_>, Vec<_>) = all_cmds
                        .into_iter()
                        .zip(all_sexps)
                        .filter(|(c, _)| match c {
                            GenericCommand::Action(GenericAction::Let(..)) => true,
                            egglog::ast::GenericCommand::Extract(..) => true,
                            egglog::ast::GenericCommand::MultiExtract(..) => true,
                            egglog::ast::GenericCommand::RunSchedule(..) => true,
                            egglog::ast::GenericCommand::PrintOverallStatistics(..) => true,
                            egglog::ast::GenericCommand::Check(..) => true,
                            egglog::ast::GenericCommand::PrintFunction(..) => true,
                            egglog::ast::GenericCommand::PrintSize(..) => true,
                            _ => false,
                        })
                        .map(|(cmd, sexp)| {
                            (
                                match cmd {
                                    GenericCommand::Action(GenericAction::Let(
                                        span,
                                        name,
                                        body,
                                    )) => GenericCommand::Action(GenericAction::Let(
                                        span,
                                        namespace.add(name),
                                        namespace.replace_expr(body),
                                    )),
                                    GenericCommand::Extract(span, e1, e2) => {
                                        GenericCommand::Extract(
                                            span,
                                            namespace.replace_expr(e1),
                                            namespace.replace_expr(e2),
                                        )
                                    }
                                    GenericCommand::MultiExtract(span, e, es) => {
                                        GenericCommand::MultiExtract(
                                            span,
                                            namespace.replace_expr(e),
                                            es.into_iter()
                                                .map(|x| namespace.replace_expr(x))
                                                .collect(),
                                        )
                                    }
                                    GenericCommand::RunSchedule(schedule) => {
                                        GenericCommand::RunSchedule(
                                            namespace.replace_sched(schedule),
                                        )
                                    }
                                    GenericCommand::PrintOverallStatistics(..) => cmd,
                                    GenericCommand::Check(span, facts) => GenericCommand::Check(
                                        span,
                                        facts
                                            .into_iter()
                                            .map(|f| namespace.replace_fact(f))
                                            .collect(),
                                    ),
                                    GenericCommand::PrintFunction(..) => cmd,
                                    GenericCommand::PrintSize(..) => cmd,
                                    _ => panic!("impossible"),
                                },
                                sexp,
                            )
                        })
                        .unzip();

                    // Run program on the mined e-graph
                    let mined_outputs = timed_egraph.run_program_with_timeline(
                        filtered_cmds,
                        &filtered_sexps
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )?;

                    let mined_extracts = collect_extract_outputs(mined_outputs);

                    add_mine_extract_summary(
                        &mut mine_extract_report,
                        name,
                        &fresh_extracts,
                        &mined_extracts,
                    )?;

                    timed_egraph.write_timeline(&out_dir.join(format!("{name}-timeline.json")))?;

                    Ok(())
                },
            );
            let file = File::create(out_dir.join("mine-extracts.json"))
                .expect("failed to create mine-extracts.json");
            serde_json::to_writer_pretty(BufWriter::new(file), &mine_extract_report)
                .expect("failed to write mine-extracts.json");
            result
        }
    }
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .parse_default_env()
        .init();
    let input_path = args.input_path.clone();
    let output_dir = args.output_dir;

    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let entries = if input_path.is_file() {
        if input_path.extension().and_then(|s| s.to_str()) == Some("egg") {
            vec![input_path]
        } else {
            panic!("input file is not an egg file")
        }
    } else if input_path.is_dir() {
        WalkDir::new(input_path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("egg"))
            .map(|entry| entry.path().to_path_buf())
            .collect()
    } else {
        panic!("Input path is neither file nor directory: {:?}", input_path);
    };

    let (success, failure) = poach(entries, &output_dir, args.run_mode, args.initial_egraph);
    #[derive(Serialize)]
    struct Output {
        success: Vec<String>,
        failure: Vec<(String, String)>,
    }
    let out = Output { success, failure };
    let file =
        File::create(output_dir.join("summary.json")).expect("Failed to create summary.json");
    serde_json::to_writer_pretty(BufWriter::new(file), &out).expect("failed to write summary.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_rules_after_deserialize() {
        let mut timed_egraph = TimedEgraph::new();
        let program = r#"
            (function fib (i64) i64 :no-merge)
            (set (fib 0) 0)
            (set (fib 1) 1)

            (rule ((= f0 (fib x))
                   (= f1 (fib (+ x 1))))
                  ((set (fib (+ x 2)) (+ f0 f1))))

            (run 2)

            (check (= (fib 3) 2))
            (fail (check (= (fib 4) 3)))
        "#;

        let res = timed_egraph.run_from_string(program);
        assert!(res.is_ok());

        // round trip serialize
        let v = timed_egraph.to_value().expect("failed to serialize");
        timed_egraph.from_value(v).expect("failed to deserialize");

        let second_run = r#"
            (fail (check (= (fib 4) 3)))
            (run 1)
            (check (= (fib 4) 3))
        "#;

        let res2 = timed_egraph.run_from_string(second_run);
        assert!(res2.is_ok());
    }
}
