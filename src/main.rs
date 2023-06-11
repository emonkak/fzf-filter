use std::borrow::Cow;
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
  -l, --limit-items NUM       a maximum number of items to output
  -f, --field-index NUM       a field index to be matched
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

    let output_content = String::from_utf8_lossy(&output.stdout);

    match (args.field_index, args.field_partitions) {
        (Some(index), Some(partitions)) => {
            let extractor = PartitionExtractor {
                index,
                partitions,
                delimiter: args.field_delimiter,
            };
            run_loop(output_content, args.limit_items, extractor);
        }
        (Some(index), None) => {
            let extractor = IndexExtractor {
                index,
                delimiter: args.field_delimiter,
            };
            run_loop(output_content, args.limit_items, extractor);
        }
        _ => {
            let extractor = ThroughExtractor;
            run_loop(output_content, args.limit_items, extractor);
        }
    }

    return Ok(ExitCode::SUCCESS);
}

fn run_loop(output_content: Cow<str>, limit_items: Option<usize>, extractor: impl Extractor) {
    let (tx, rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let stdin = io::stdin();
        let mut buffer = String::new();
        while let Ok(num_bytes) = stdin.read_line(&mut buffer) {
            if num_bytes == 0 {
                break;
            }
            tx.send(mem::take(&mut buffer)).unwrap();
        }
    });

    let slab = fzf::Slab::default();

    while let Ok(line) = rx.recv() {
        let line = rx.try_iter().last().unwrap_or(line);
        let Some((sequence, pattern)) = line.trim_end_matches('\n').split_once(' ') else {
            continue;
        };
        if pattern.is_empty() {
            if let Some(limit_items) = limit_items {
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

            for line in output_content.lines() {
                if let Some(content) = extractor.extract(line) {
                    let score = fzf::get_score(content, &pattern, &slab);
                    if score > 0 {
                        matched_lines.push((Reverse(score), line));
                    }
                }
            }

            let matched_lines = match limit_items {
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

trait Extractor {
    fn extract<'a>(&self, s: &'a str) -> Option<&'a str>;
}

struct PartitionExtractor {
    index: usize,
    partitions: usize,
    delimiter: char,
}

impl Extractor for PartitionExtractor {
    fn extract<'a>(&self, s: &'a str) -> Option<&'a str> {
        s.splitn(self.partitions, self.delimiter).nth(self.index)
    }
}

struct IndexExtractor {
    index: usize,
    delimiter: char,
}

impl Extractor for IndexExtractor {
    fn extract<'a>(&self, s: &'a str) -> Option<&'a str> {
        s.split(self.delimiter).nth(self.index)
    }
}

struct ThroughExtractor;

impl Extractor for ThroughExtractor {
    fn extract<'a>(&self, s: &'a str) -> Option<&'a str> {
        return Some(s);
    }
}
