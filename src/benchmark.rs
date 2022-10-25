use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{cmp::AllocLimits, trace_allocs, MemoryStats};
use clap::Parser;
use serde::Deserialize;

const DIR: &str = "mem_bench";

#[derive(Debug, Parser)]
struct MemBenchArgs {
    #[arg(short, long, value_name = "DIR")]
    load_baseline: Option<PathBuf>,
    #[arg(short, long, value_name = "DIR")]
    save_baseline: Option<PathBuf>,
    #[arg(short, long)]
    discard_baseline: bool,
}

fn parse_args() -> MemBenchArgs {
    MemBenchArgs::parse_from(env::args().skip_while(|a| a != "--").skip(1))
}

fn load_stats(path: &Path) -> Option<MemoryStats> {
    let content = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => panic!("cannot load stats from {path:?}: {e}"),
    };
    let v = toml::from_str(&content)
        .unwrap_or_else(|e| panic!("cannot parse stats from {path:?}: {e}"));
    Some(v)
}

fn store_stats(stats: &MemoryStats, path: &Path) {
    let stats = toml::to_string(stats)
        .unwrap_or_else(|e| panic!("cannot unparse stats into {path:?}: {e}"));

    match path.parent() {
        None => unreachable!("cannot gen parent of {path:?}"),
        Some(p) if !p.exists() => {
            fs::create_dir_all(p).unwrap_or_else(|e| panic!("cannot create directory {p:?}: {e}"))
        }
        _ => {}
    }

    println!("storing stats as {path:?}");

    fs::write(path, stats.as_bytes())
        .unwrap_or_else(|e| panic!("cannot store stats into {path:?}: {e}"));
}

fn baseline_file(path: &Path, id: &str) -> PathBuf {
    path.join(id).with_extension("toml")
}

/// Returns the Cargo target directory, possibly calling `cargo metadata` to
/// figure it out.
fn cargo_target_directory() -> Option<PathBuf> {
    #[derive(Deserialize, Debug)]
    struct Metadata {
        target_directory: PathBuf,
    }

    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let output = Command::new(env::var_os("CARGO")?)
                .args(&["metadata", "--format-version", "1"])
                .output()
                .ok()?;
            let metadata: Metadata = serde_json::from_slice(&output.stdout).ok()?;
            Some(metadata.target_directory)
        })
}

fn default_dir() -> PathBuf {
    cargo_target_directory()
        .unwrap_or_else(PathBuf::new)
        .join(DIR)
}

pub fn mem_bench<F: FnOnce() -> O, O>(id: &str, limits: &AllocLimits, f: F) -> MemoryStats {
    let args = parse_args();
    let ref_stats = if !args.discard_baseline {
        load_stats(&baseline_file(
            &args.load_baseline.unwrap_or_else(default_dir),
            id,
        ))
    } else {
        None
    };

    let (_, stats) = trace_allocs(f);

    if let Some(ref_stats) = ref_stats {
        limits
            .check(&stats, &ref_stats)
            .unwrap_or_else(|e| panic!("regression in test `{id}` detected: {e}"));
    }

    if !args.discard_baseline {
        store_stats(
            &stats,
            &baseline_file(&args.save_baseline.unwrap_or_else(default_dir), id),
        )
    }

    stats
}

