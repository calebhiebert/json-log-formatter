use std::collections::HashSet;
use std::fmt::format;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::process::exit;
use std::ptr::write;
use atty::Stream;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use clap::Parser;
use serde_json::{Map, Value};
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

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
    let cli = Args::parse();

    if atty::is(Stream::Stdin) {
        println!("In order to use this utility, data must be piped to stdin");
        exit(1);
    }

    let msg_field_name = match (&cli.message_field_name).as_ref() {
        Some(field_name) => field_name,
        None => "msg"
    };

    let level_field_name = match (&cli.level_field_name).as_ref() {
        Some(field_name) => field_name,
        None => "level"
    };

    let timestamp_field_name = match (&cli.timestamp_field).as_ref() {
        Some(field_name) => field_name,
        None => "ts"
    };

    let mut excluded_fields: HashSet<String> = match (&cli.exclude_fields).as_ref() {
        Some(excluded) => excluded.into_iter().cloned().collect(),
        None => HashSet::new(),
    };

    let separator = match (&cli.separator).as_ref() {
        Some(sep) => sep,
        None => "|"
    };

    excluded_fields.insert(msg_field_name.to_string());
    excluded_fields.insert(level_field_name.to_string());
    excluded_fields.insert(timestamp_field_name.to_string());

    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = line.expect("Could not read from standard in");

        match serde_json::from_str::<Value>(&line) {
            Ok(val) => {
                match val.as_object() {
                    Some(obj) => {
                        let message = obj.get(msg_field_name).unwrap().as_str().unwrap_or("???");
                        let level = obj.get(level_field_name).unwrap().as_str().unwrap_or("???");

                        let time = obj.get(timestamp_field_name).unwrap().as_f64();

                        let extra_fields: Vec<(String, String)> = obj.iter()
                            .filter(|(k, v)| !excluded_fields.contains(k.clone()))
                            .map(|(k, v)| {
                                let formatted_value = match v {
                                    Value::Null => "NULL".to_string(),
                                    Value::Bool(b) => match b {
                                        true => "true".to_string(),
                                        false => "false".to_string()
                                    }
                                    Value::Number(n) => {
                                        n.to_string()
                                    }
                                    Value::String(s) => s.clone(),
                                    Value::Array(a) => format!("{:?}", &a),
                                    Value::Object(o) => format!("{:?}", &o)
                                };

                                (k.clone(), formatted_value.clone())
                            }).collect();

                        let mut bufwtr = BufferWriter::stderr(ColorChoice::Always);
                        let mut buffer = bufwtr.buffer();

                        macro_rules! col {
                            ($col:expr) => {
                                buffer.set_color(ColorSpec::new().set_fg(Some($col)));
                            };
                        }

                        if let Some(t) = time {
                            let parsed_dt = NaiveDateTime::from_timestamp(t as i64, 0);
                            let datetime: DateTime<Utc> = DateTime::from_utc(parsed_dt, Utc);
                            let local_dt: DateTime<Local> = DateTime::from(datetime);

                            col!(Color::Magenta);
                            write!(&mut buffer, "[{}]", local_dt.format("%Y-%m-%d %r"));
                        }

                        col!(match level {
                            "trace" | "debug" => Color::Black,
                            "info" => Color::Blue,
                            "warning" => Color::Yellow,
                            "error" | "critical" => Color::Red,
                            _ => Color::Black
                        });

                        write!(&mut buffer, "[{}] ", level);

                        col!(Color::Black);
                        write!(&mut buffer, "{}", message);

                        extra_fields.iter().for_each(|(k, v)| {
                            col!(Color::Black);
                            write!(&mut buffer, " {} ", separator);
                            col!(Color::Green);
                            write!(&mut buffer, "{}", k);
                            col!(Color::Black);
                            write!(&mut buffer, "=");
                            col!(Color::Black);
                            write!(&mut buffer, "{}", v);
                        });

                        col!(Color::Black);
                        write!(&mut buffer, "\n");

                        bufwtr.print(&buffer);
                    }
                    None => {
                        println!("{}", line);
                    }
                }
            }

            Err(_) => {
                println!("{}", line);
            }
        };
    }

    println!("{:?}", cli);
}
