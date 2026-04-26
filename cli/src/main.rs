use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

/// Apply a fixed gain (in 1.5 dB AAC native steps) to an M4A/MP4 file.
///
/// Use `-` as INPUT to read from stdin (requires faststart input — moov before
/// mdat) and/or as OUTPUT to write to stdout. The source file is never
/// overwritten.
#[derive(Parser, Debug)]
#[command(
    name = "m4againrs",
    version,
    about,
    long_about = None,
    allow_negative_numbers = true,
)]
struct Cli {
    /// Source M4A path, or `-` to read from stdin.
    input: String,
    /// Destination M4A path, or `-` to write to stdout.
    output: String,
    /// Signed integer gain steps (one step = 1.5 dB). Must be non-zero.
    #[arg(long = "gain-steps", value_name = "STEPS", allow_hyphen_values = true)]
    gain_steps: i32,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli.input, &cli.output, cli.gain_steps) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &str, output: &str, gain_steps: i32) -> m4againrs::Result<usize> {
    let stdin_input = input == "-";
    let stdout_output = output == "-";

    match (stdin_input, stdout_output) {
        (true, true) => {
            let stdin = io::stdin();
            let mut src = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut dst = BufWriter::new(stdout.lock());
            m4againrs::aac_apply_gain_streaming(&mut src, &mut dst, gain_steps)
        }
        (true, false) => {
            let stdin = io::stdin();
            let mut src = BufReader::new(stdin.lock());
            let mut dst = BufWriter::new(File::create(output)?);
            m4againrs::aac_apply_gain_streaming(&mut src, &mut dst, gain_steps)
        }
        (false, true) => {
            let mut src = File::open(input)?;
            let stdout = io::stdout();
            let mut dst = BufWriter::new(stdout.lock());
            m4againrs::aac_apply_gain_to_writer(&mut src, &mut dst, gain_steps)
        }
        (false, false) => {
            m4againrs::aac_apply_gain_file(Path::new(input), Path::new(output), gain_steps)
        }
    }
}
