use anyhow::{anyhow, Context as _};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Reverse;
use std::env::{self, ArgsOs};
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::process::{Command, ExitCode};

const HELP: &'static str = "\
USAGE:
  fuzzy-filter [OPTIONS] -- <COMMAND> [COMMAND_ARGUMENTS]

OPTIONS:
  -l, --limit-items <N>  a maximum number of items to print
  -h, --help             Print help information";

fn main() -> anyhow::Result<ExitCode> {
    let args = parse_args(env::args_os());
    match args {
        Ok(args) => {
            if args.help {
                println!("{}", HELP);
                Ok(ExitCode::SUCCESS)
            } else {
                run(args)
            }
        }
        Err(error) => {
            println!("{}", HELP);
            eprintln!("Error: {}", error);
            Ok(ExitCode::FAILURE)
        }
    }
}

fn run(args: Args) -> anyhow::Result<ExitCode> {
    let output = Command::new(args.command)
        .args(args.command_args)
        .output()?;
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Ok(ExitCode::FAILURE);
    }

    let matcher = SkimMatcherV2::default();
    let stdin = io::stdin();

    let output_content = String::from_utf8(output.stdout)
        .context("failed to parse the command output as a UTF8 string")?;
    let mut pattern_buffer = String::new();
    let mut sequence = 1;

    while let Ok(num_bytes) = stdin.read_line(&mut pattern_buffer) {
        if num_bytes == 0 {
            break;
        }
        let pattern = pattern_buffer.trim();
        if pattern.is_empty() {
            if let Some(limit_items) = args.limit_items {
                for line in output_content.lines().take(limit_items) {
                    println!("{} {}", sequence, line)
                }
            } else {
                for line in output_content.lines() {
                    println!("{} {}", sequence, line)
                }
            }
        } else {
            let mut matched_lines = vec![];
            for line in output_content.lines() {
                if let Some(score) = matcher.fuzzy_match(&line, pattern) {
                    matched_lines.push(Reverse((score, line)));
                }
            }
            match args.limit_items {
                Some(limit_items) if matched_lines.len() > limit_items => {
                    let (top_items, _, _) = matched_lines.select_nth_unstable(limit_items);
                    top_items.sort_unstable();
                    for Reverse((_, line)) in top_items {
                        println!("{} {}", sequence, line);
                    }
                }
                _ => {
                    matched_lines.sort_unstable();
                    for Reverse((_, line)) in matched_lines {
                        println!("{} {}", sequence, line);
                    }
                }
            }
        }
        println!("{}", sequence); // EOF
        pattern_buffer.clear();
        sequence += 1;
    }

    return Ok(ExitCode::SUCCESS);
}

#[derive(Debug)]
struct Args {
    limit_items: Option<usize>,
    command: OsString,
    command_args: Vec<OsString>,
    help: bool,
}

fn parse_args(args: ArgsOs) -> anyhow::Result<Args> {
    let mut head_parts = vec![];
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg.to_str().map_or(false, |s| s == "--") {
            break;
        }
        head_parts.push(arg)
    }
    let command = iter.next().ok_or(anyhow!("command not specified"))?;
    let command_args = iter.collect();
    let mut pico_args = pico_args::Arguments::from_vec(head_parts);
    Ok(Args {
        command,
        command_args,
        limit_items: pico_args.opt_value_from_str(["-l", "--limit-items"])?,
        help: pico_args.contains(["-h", "--help"]),
    })
}
