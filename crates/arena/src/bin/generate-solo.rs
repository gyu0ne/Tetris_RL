use arena::{SoloGenerationConfig, generate_solo_dataset};
use std::env;
use std::path::PathBuf;

fn main() {
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("generate-solo: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return Ok(());
    }
    let config = SoloGenerationConfig {
        records_path: PathBuf::from(required(&args, "--records")?),
        manifest_path: PathBuf::from(required(&args, "--manifest")?),
        engine_revision: required(&args, "--engine-revision")?,
        base_seed: parse(&args, "--seed", 1_u64)?,
        matches: parse(&args, "--matches", 8_u32)?,
        decisions_per_match: parse(&args, "--decisions-per-match", 128_u32)?,
    };
    let summary = generate_solo_dataset(&config).map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&summary.manifest).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn required(args: &[String], flag: &str) -> Result<String, String> {
    value(args, flag)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing required {flag}"))
}

fn parse<T>(args: &[String], flag: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value(args, flag).map_or(Ok(default), |raw| {
        raw.parse::<T>()
            .map_err(|error| format!("invalid {flag} value {raw:?}: {error}"))
    })
}

fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn print_usage() {
    println!(
        "Usage: generate-solo --records PATH --manifest PATH --engine-revision REV \\\n+         [--seed N] [--matches N] [--decisions-per-match N]"
    );
}
