use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "Usage: m4againrs <input.m4a|-> <output.m4a|-> <gain_steps>";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);

    let Some(input) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(output) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(steps_arg) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    let gain_steps: i32 = match steps_arg.parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("error: <gain_steps> must be an integer (got {steps_arg:?})");
            return ExitCode::from(2);
        }
    };

    match run(&input, &output, gain_steps) {
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
