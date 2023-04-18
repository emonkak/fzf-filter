use anyhow::{anyhow, Context as _};
use std::cmp::Reverse;
use std::env::{self, ArgsOs};
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::process::{Command, ExitCode};

use fzf_filter::fzf;

const HELP: &'static str = "\
USAGE:
  fzf-filter [OPTIONS] -- <COMMAND> [COMMAND_ARGUMENTS]

OPTIONS:
  -l, --limit-items <N>  a maximum number of items to print
  -h, --help             Print help information";

fn main() -> anyhow::Result<ExitCode> {
    let args = Args::parse(env::args_os());
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

    let stdin = io::stdin();
    let slab = fzf::Slab::default();

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
            let pattern = fzf::Pattern::new(pattern, fzf::CaseMode::Smart, true);

            let mut matched_lines = vec![];

            match args.field {
                Some(field) => {
                    for line in output_content.lines() {
                        if let Some(content) = line.split('\t').nth(field) {
                            let score = fzf::get_score(content, &pattern, &slab);
                            if score > 0 {
                                matched_lines.push(Reverse((score, line)));
                            }
                        }
                    }
                }
                None => {
                    for line in output_content.lines() {
                        let score = fzf::get_score(line, &pattern, &slab);
                        if score > 0 {
                            matched_lines.push(Reverse((score, line)));
                        }
                    }
                }
            }

            let matched_lines = match args.limit_items {
                Some(limit_items) if matched_lines.len() > limit_items => {
                    let (partial_lines, _, _) = matched_lines.select_nth_unstable(limit_items);
                    partial_lines
                }
                _ => matched_lines.as_mut_slice(),
            };
            matched_lines.sort_unstable();
            for Reverse((_, line)) in matched_lines {
                println!("{} {}", sequence, line);
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
    command: OsString,
    command_args: Vec<OsString>,
    field: Option<usize>,
    help: bool,
    limit_items: Option<usize>,
}

impl Args {
    fn parse(args: ArgsOs) -> anyhow::Result<Self> {
        let mut head_parts = vec![];
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            if arg.eq_ignore_ascii_case("--") {
                break;
            }
            head_parts.push(arg)
        }
        let command = iter.next().ok_or(anyhow!("command is not specified"))?;
        let command_args = iter.collect();
        let mut pico_args = pico_args::Arguments::from_vec(head_parts);
        Ok(Self {
            command,
            command_args,
            field: pico_args.opt_value_from_str(["-f", "--field"])?,
            help: pico_args.contains(["-h", "--help"]),
            limit_items: pico_args.opt_value_from_str(["-l", "--limit-items"])?,
        })
    }
}
