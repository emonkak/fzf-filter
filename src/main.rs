use std::cmp::Reverse;
use std::env::{self, ArgsOs};
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::mem;
use std::process::{Command, ExitCode};
use std::sync::mpsc;
use std::thread;

use fzf_filter::fzf;

const HELP: &'static str = "\
USAGE:
  fzf-filter [OPTIONS] -- <COMMAND> [COMMAND_ARGUMENTS]

OPTIONS:
  -l, --limit-items NUM       a maximum number of items to print
  -f, --field-index INDEX     a field index to be matched
                              (default: whole line)
  -p, --field-partitions NUM  a maximum number of partitions of the field
  -d, --field-delimiter CHAR  a field delimiter character
                              (default: \\t)";

fn main() -> anyhow::Result<ExitCode> {
    let mode = Args::parse(env::args_os());
    match mode {
        Ok(Some(args)) => run(args),
        Ok(None) => {
            println!("{}", HELP);
            Ok(ExitCode::SUCCESS)
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
    let (tx, rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let mut buffer = String::new();
        while let Ok(num_bytes) = stdin.read_line(&mut buffer) {
            if num_bytes == 0 {
                break;
            }
            tx.send(mem::take(&mut buffer)).unwrap();
        }
    });

    let output_content = String::from_utf8_lossy(&output.stdout);
    let slab = fzf::Slab::default();

    while let Ok(line) = rx.recv() {
        let line = rx.try_iter().last().unwrap_or(line);
        let Some((sequence, pattern)) = line.trim_end_matches('\n').split_once(' ') else {
            continue;
        };
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

            match args.field_index {
                Some(index) => {
                    let delimiter = args.field_delimiter;
                    let partitions = args.field_partitions;
                    for line in output_content.lines() {
                        if let Some(content) = extract_field(line, delimiter, index, partitions) {
                            let score = fzf::get_score(content, &pattern, &slab);
                            if score > 0 {
                                matched_lines.push((Reverse(score), line));
                            }
                        }
                    }
                }
                None => {
                    for line in output_content.lines() {
                        let score = fzf::get_score(line, &pattern, &slab);
                        if score > 0 {
                            matched_lines.push((Reverse(score), line));
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

            for (_, line) in matched_lines {
                println!("{} {}", sequence, line);
            }
        }
        println!("{}", sequence); // EOF
    }

    return Ok(ExitCode::SUCCESS);
}

#[derive(Debug)]
struct Args {
    command: OsString,
    command_args: Vec<OsString>,
    field_delimiter: char,
    field_index: Option<usize>,
    field_partitions: Option<usize>,
    limit_items: Option<usize>,
}

impl Args {
    fn parse(args: ArgsOs) -> anyhow::Result<Option<Self>> {
        let mut iter = args.into_iter();
        let mut main_args = vec![];
        while let Some(arg) = iter.next() {
            if arg.eq_ignore_ascii_case("--") {
                break;
            }
            main_args.push(arg)
        }
        let mut pico_args = pico_args::Arguments::from_vec(main_args);
        let Some(command) = iter.next() else {
            return Ok(None);
        };
        let command_args = iter.collect();
        Ok(Some(Args {
            command,
            command_args,
            field_delimiter: pico_args
                .opt_value_from_str(["-d", "--field-delimiter"])?
                .unwrap_or('\t'),
            field_index: pico_args.opt_value_from_str(["-f", "--field-index"])?,
            field_partitions: pico_args.opt_value_from_str(["-p", "--field-partitions"])?,
            limit_items: pico_args.opt_value_from_str(["-l", "--limit-items"])?,
        }))
    }
}

fn extract_field(
    s: &str,
    delimiter: char,
    index: usize,
    partitions: Option<usize>,
) -> Option<&str> {
    match partitions {
        Some(n) => s.splitn(n, delimiter).nth(index),
        None => s.split(delimiter).nth(index),
    }
}
