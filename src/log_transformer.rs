use std::collections::HashSet;
use std::io::Write;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use clap::Parser;
use serde_json::Value;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Config {
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

    #[clap(short, long)]
    hide_extra_fields: Option<bool>,

    #[clap(short, long)]
    filter_levels: Option<Vec<String>>,

    #[clap(short, long)]
    disable_colors: Option<bool>,

    timestamp_format: Option<String>,
}

pub struct LogTransformer {
    config: Config,

    message_field: String,
    level_field: String,
    timestamp_field: String,
    excluded_fields: HashSet<String>,
    separator: String,
    hide_extra_fields: bool,
    filter_levels: HashSet<String>,
    disable_colors: bool,
}

impl LogTransformer {
    pub fn new(config: Config) -> LogTransformer {
        let msg_field_name = match (&config.message_field_name).as_ref() {
            Some(field_name) => field_name.clone(),
            None => "msg".to_string()
        };

        let level_field_name = match (&config.level_field_name).as_ref() {
            Some(field_name) => field_name.clone(),
            None => "level".to_string()
        };

        let timestamp_field_name = match (&config.timestamp_field).as_ref() {
            Some(field_name) => field_name.clone(),
            None => "ts".to_string()
        };

        let mut excluded_fields: HashSet<String> = match (&config.exclude_fields).as_ref() {
            Some(excluded) => excluded.into_iter().cloned().collect(),
            None => HashSet::new(),
        };

        let filter_levels: HashSet<String> = match (&config.filter_levels).as_ref() {
            Some(filtered) => filtered.into_iter().cloned().collect(),
            None => HashSet::new(),
        };

        let separator = match (&config.separator).as_ref() {
            Some(sep) => sep.clone(),
            None => "|".to_string()
        };

        excluded_fields.insert(msg_field_name.to_string());
        excluded_fields.insert(level_field_name.to_string());
        excluded_fields.insert(timestamp_field_name.to_string());

        LogTransformer {
            message_field: msg_field_name,
            level_field: level_field_name,
            timestamp_field: timestamp_field_name,
            excluded_fields,
            separator,
            hide_extra_fields: *(&config).hide_extra_fields.as_ref().unwrap_or(&false),
            filter_levels,
            disable_colors: *(&config).disable_colors.as_ref().unwrap_or(&false),
            config,
        }
    }

    pub fn transform_and_print(&self, line: String) -> anyhow::Result<()> {
        match serde_json::from_str::<Value>(&line) {
            Ok(val) => {
                match val.as_object() {
                    Some(obj) => {
                        let message = obj.get(self.message_field.as_str()).unwrap().as_str().unwrap_or("???");
                        let level = obj.get(self.level_field.as_str()).unwrap().as_str().unwrap_or("???");

                        let time = obj.get(self.timestamp_field.as_str()).unwrap().as_f64();

                        let bufwtr = BufferWriter::stdout(match self.disable_colors {
                            true => ColorChoice::Never,
                            false => ColorChoice::Auto,
                        });

                        let mut buffer = bufwtr.buffer();

                        macro_rules! col {
                            ($col:expr) => {
                                buffer.set_color(ColorSpec::new().set_fg(Some($col)))?;
                            };
                        }

                        if let Some(t) = time {
                            let parsed_dt = NaiveDateTime::from_timestamp(t as i64, 0);
                            let datetime: DateTime<Utc> = DateTime::from_utc(parsed_dt, Utc);
                            let local_dt: DateTime<Local> = DateTime::from(datetime);

                            col!(Color::Magenta);
                            write!(&mut buffer, "[{}]", local_dt.format("%Y-%m-%d %r"))?;
                        }

                        col!(match level {
                            "trace" | "debug" => Color::Black,
                            "info" => Color::Blue,
                            "warning" => Color::Yellow,
                            "error" | "critical" => Color::Red,
                            _ => Color::Black
                        });

                        write!(&mut buffer, "[{}] ", level).unwrap();

                        col!(Color::Black);
                        write!(&mut buffer, "{}", message).unwrap();

                        if !self.hide_extra_fields {
                            let extra_fields: Vec<(String, String)> = obj.iter()
                                .filter(|(k, _)| !self.excluded_fields.contains(k.clone()))
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

                            for (k, v) in extra_fields {
                                col!(Color::Black);
                                write!(&mut buffer, " {} ", self.separator)?;
                                col!(Color::Green);
                                write!(&mut buffer, "{}", k)?;
                                col!(Color::Black);
                                write!(&mut buffer, "=")?;
                                col!(Color::Black);
                                write!(&mut buffer, "{}", v)?;
                            }
                        }

                        col!(Color::Black);
                        write!(&mut buffer, "\n")?;

                        bufwtr.print(&buffer)?;
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

        Ok(())
    }
}