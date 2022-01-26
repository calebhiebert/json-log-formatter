mod log_transformer;

use std::io;
use std::io::BufRead;
use std::process::exit;
use atty::Stream;

use clap::Parser;
use crate::log_transformer::{Config, LogTransformer};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    message_field_name: Option<String>,

    #[clap(short, long)]
    level_field_name: Option<String>,

    #[clap(short, long)]
    exclude_fields: Option<Vec<String>>,

    #[clap(short, long)]
    separator: Option<String>,

    #[clap(short, long)]
    timestamp_field: Option<String>,

    timestamp_format: Option<String>,
}

fn main() {
    let config = Config::parse();

    if atty::is(Stream::Stdin) {
        println!("In order to use this utility, data must be piped to stdin");
        exit(1);
    }

    let stdin = io::stdin();

    let transformer = LogTransformer::new(config);

    for line in stdin.lock().lines() {
        let line = line.expect("Could not read from standard in");
        transformer.transform_and_print(line).unwrap();
    }
}
