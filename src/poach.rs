use clap::{Parser, ValueEnum};
use egglog::TimedEgraph;
use env_logger::Env;
use hashbrown::HashMap;

use std::fmt::Debug;
use std::fs::{self, read_to_string};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Debug)]
enum RunMode {
    // For each egg file under the input path,
    //      run the egglog program and record timing information. Do not serialize.
    //      Save the complete timeline, for consumption by the nightly frontend.
    TimelineOnly,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph, recording timing information.
    //      Deserialize the serialized egraph, recording timing information.
    //      Assert the deserialized egraph has the same size as the initial egraph
    //      Save the complete timeline, for consumption by the nightly frontend.
    SequentialRoundTrip,

    // For each egg file under the input path,
    //      Run the egglog program, recording timing information.
    //      Serialize the resulting egraph
    // For each egg file under the input path,
    //      Deserialize the deserialized egraph
    //      Assert the deserialized egraph has the same size as the initial egraph
    // Save the complete timeline, for consumption by the nightly frontend.
    InterleavedRoundTrip,

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
}

#[derive(Debug, Parser)]
#[command(version = env!("FULL_VERSION"), about= env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    input_path: PathBuf,
    output_dir: PathBuf,
    run_mode: RunMode,
}

fn check_egraph_size(egraph: &TimedEgraph) {
    let expected = egraph.num_tuples();
    for eg in egraph.egraphs().iter() {
        assert!(eg.num_tuples() == expected);
    }
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
        let file = fs::File::create(out_dir.join("diff.json")).expect("Failed to create diff file");
        serde_json::to_writer_pretty(file, &diff).expect("failed to serialize diff");
        panic!("Diff for {}", name)
    }
}

fn run_egg_file(egg_file: &PathBuf) -> TimedEgraph {
    let mut egraph = TimedEgraph::new();
    let filename = egg_file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    egraph
        .parse_and_run_program(
            filename,
            &read_to_string(egg_file).expect(&format!("Failed to open {}", egg_file.display())),
        )
        .expect("fail");

    egraph
}

fn poach(files: Vec<PathBuf>, out_dir: &PathBuf, run_mode: RunMode) {
    match run_mode {
        RunMode::TimelineOnly => {
            for (idx, file) in files.iter().enumerate() {
                let name = file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                println!("[{}/{}] {}", idx, files.len(), name);
                let out_dir = out_dir.join(file.file_stem().unwrap().to_str().unwrap());
                let egraph = run_egg_file(&file);
                egraph.write_timeline(&out_dir).expect("fail");
            }
        }
        RunMode::SequentialRoundTrip => {
            for (idx, file) in files.iter().enumerate() {
                let name = file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                println!("[{}/{}] {}", idx, files.len(), name);
                let out_dir = out_dir.join(file.file_stem().unwrap().to_str().unwrap());
                let mut egraph = run_egg_file(&file);
                let s1 = out_dir.join("serialize1.json");

                egraph
                    .serialize_egraph(&s1)
                    .expect("failed to serialize s1.json");

                egraph
                    .deserialize_egraph(&s1)
                    .expect("failed to read s1.json");

                assert!(egraph.egraphs().len() == 2);
                check_egraph_size(&egraph);

                egraph.write_timeline(&out_dir).expect("fail");
            }
        }
        RunMode::InterleavedRoundTrip => {
            let mut tmp = HashMap::new();
            for (idx, file) in files.iter().enumerate() {
                let name = file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                println!("[{}/{}](Part 1) {}", idx, files.len(), name);
                let out_dir = out_dir.join(file.file_stem().unwrap().to_str().unwrap());
                let mut egraph = run_egg_file(&file);
                let s1 = out_dir.join("serialize1.json");
                egraph
                    .serialize_egraph(&s1)
                    .expect("failed to serialize s1.json");
                tmp.insert(file, (out_dir, egraph));
            }
            for (idx, file) in files.iter().enumerate() {
                let name = file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                println!("[{}/{}](Part 2) {}", idx, files.len(), name);
                let (out_dir, egraph) = tmp.get_mut(file).unwrap();
                egraph
                    .deserialize_egraph(&out_dir.join("serialize1.json"))
                    .expect("failed to deserialize s1.json");

                assert!(egraph.egraphs().len() == 2);
                check_egraph_size(&egraph);

                egraph.write_timeline(&out_dir).expect("fail");
            }
        }
        RunMode::IdempotentRoundTrip => {
            for (idx, file) in files.iter().enumerate() {
                let name = file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                println!("[{}/{}] {}", idx, files.len(), name);
                let out_dir = out_dir.join(file.file_stem().unwrap().to_str().unwrap());
                let mut egraph = run_egg_file(&file);
                let s1 = out_dir.join("serialize1.json");
                let s2 = out_dir.join("serialize2.json");
                let s3 = out_dir.join("serialize3.json");

                egraph
                    .serialize_egraph(&s1)
                    .expect("failed to serialize s1.json");

                egraph
                    .deserialize_egraph(&s1)
                    .expect("failed to read s1.json");

                egraph
                    .serialize_egraph(&s2)
                    .expect("failed to serialize s2.json");

                egraph
                    .deserialize_egraph(&s2)
                    .expect("failed to read s2.json");

                egraph
                    .serialize_egraph(&s3)
                    .expect("failed to serialize s3.json");

                egraph
                    .deserialize_egraph(&s3)
                    .expect("failed to read s3.json");

                assert!(egraph.egraphs().len() == 4);
                check_egraph_size(&egraph);
                check_idempotent(&s2, &s3, name, &out_dir);

                egraph.write_timeline(&out_dir).expect("fail");
            }
        }
        RunMode::OldSerialize => {
            for (idx, file) in files.iter().enumerate() {
                let name = file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                println!("[{}/{}] {}", idx, files.len(), name);
                let out_dir = out_dir.join(file.file_stem().unwrap().to_str().unwrap());
                let mut egraph = run_egg_file(&file);

                egraph
                    .serialize_egraph(&out_dir.join("serialize-poach.json"))
                    .expect("failed to serialize poach.json");

                egraph
                    .old_serialize_egraph(&out_dir.join("serialize-old.json"))
                    .expect("failed to serialize old.json");

                egraph.write_timeline(&out_dir).expect("fail");
            }
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
            .filter(|entry| !entry.path().to_string_lossy().contains("fail"))
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("egg"))
            .map(|entry| entry.path().to_path_buf())
            .collect()
    } else {
        panic!("Input path is neither file nor directory: {:?}", input_path);
    };

    poach(entries, &args.output_dir, args.run_mode);
}
