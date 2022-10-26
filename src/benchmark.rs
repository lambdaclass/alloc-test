use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{cmp::AllocLimits, trace_allocs, MemoryStats};
use clap::Parser;
use serde::Deserialize;

const DIR: &str = "mem_bench";

#[derive(Debug, thiserror::Error)]
pub enum BenchmarkError {
    #[error("regression detected: {_0}")]
    Regression(#[from] crate::cmp::AllocLimitsError),
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error(transparent)]
    Decode(#[from] toml::de::Error),
}

#[derive(Debug, Parser)]
struct MemBenchArgs {
    #[arg(short, long, value_name = "DIR", env)]
    load_baseline: Option<PathBuf>,
    #[arg(short, long, value_name = "DIR")]
    save_baseline: Option<PathBuf>,
    #[arg(short, long)]
    discard_baseline: bool,
}

fn parse_args() -> MemBenchArgs {
    let (test_n, exact) =
        env::args()
            .skip(1)
            .take_while(|a| a != "--")
            .fold((0, false), |(n, e), a| match a.as_str() {
                "--exact" => (n, true),
                _ if !a.starts_with("-") => (n + 1, e),
                _ => (n, e),
            });
    if test_n != 1 {
        panic!("specify exactly one test to run");
    }
    if !exact {
        panic!("make sure only one test is executed by adding `--exact` parameter")
    }
    // TODO replace argv[0] with something sensible
    MemBenchArgs::parse_from(env::args().skip_while(|a| a != "--"))
}

fn load_stats(path: &Path, fail_on_not_found: bool) -> Result<Option<MemoryStats>, BenchmarkError> {
    let content = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) if !fail_on_not_found && e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let v = toml::from_str(&content)?;
    Ok(Some(v))
}

fn store_stats(stats: &MemoryStats, path: &Path) -> Result<(), BenchmarkError> {
    // shouldn't panic unless `MemoryStats` contains unsupported data types
    let stats = toml::to_string(stats)
        .unwrap_or_else(|e| unreachable!("cannot unparse stats into toml: {e}\ndata: {stats:#?}"));

    match path.parent() {
        None => unreachable!("cannot gen parent of {path:?}"),
        Some(p) if !p.exists() => fs::create_dir_all(p)?,
        _ => {}
    }

    fs::write(path, stats.as_bytes())?;
    Ok(())
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

pub fn mem_bench<F: FnOnce() -> O, O>(
    id: &str,
    limits: &AllocLimits,
    f: F,
) -> Result<MemoryStats, BenchmarkError> {
    let args = parse_args();
    let ref_stats = if let Some(load_baseline) = &args.load_baseline {
        load_stats(&baseline_file(load_baseline, id), true)?
    } else {
        load_stats(&baseline_file(&default_dir(), id), false)?
    };

    let (_, stats) = trace_allocs(f);

    if let Some(ref_stats) = ref_stats {
        limits.check(&stats, &ref_stats)?;
    }

    if !args.discard_baseline && args.load_baseline.is_none() {
        store_stats(
            &stats,
            &baseline_file(&args.save_baseline.unwrap_or_else(default_dir), id),
        )?;
    }

    println!("memory allocation stats for `{id}`:\n{stats}\n");

    Ok(stats)
}

#[macro_export]
macro_rules! mem_bench {
    ($test:ident, $limits:expr) => {
        crate::mem_bench(stringify!($test), $limits, $test)
    };
}
