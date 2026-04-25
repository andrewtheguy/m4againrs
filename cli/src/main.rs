use std::env;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "Usage: m4againrs <input.m4a> <output.m4a|-> <gain_steps>";

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
    if output == "-" {
        let mut src = File::open(input)?;
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        return m4againrs::aac_apply_gain_to_writer(&mut src, &mut stdout, gain_steps);
    }

    m4againrs::aac_apply_gain_file(Path::new(input), Path::new(output), gain_steps)
}
