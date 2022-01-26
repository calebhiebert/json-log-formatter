use std::collections::HashSet;
use std::io::Write;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use clap::Parser;
use serde_json::{Map, Value};
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

/// Log messages longer than MULTILINE_MESSAGE_THRESHOLD will have their fields put on a separate line
const MULTILINE_MESSAGE_THRESHOLD: usize = 120;

/// Extra fields longer than MULTILINE_FIELD_THRESHOLD will be put on their own line
const MULTILINE_FIELD_THRESHOLD: usize = 120;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Config {
    /// The name of the json field containing the message
    #[clap(short, long, default_value = "msg")]
    message_field_name: String,

    /// The name of the json field containing the level
    #[clap(short, long, default_value = "level")]
    level_field_name: String,

    /// A list of json fields to exclude from logging. Default none
    #[clap(short, long)]
    exclude_fields: Option<Vec<String>>,

    /// The separator printed between extra json fields
    #[clap(long, default_value = "|")]
    separator: String,

    /// The name of the json field containing the timestamp
    #[clap(short, long, default_value = "ts")]
    timestamp_field_name: String,

    /// Does not print extra json fields
    #[clap(short, long)]
    hide_extra_fields: bool,

    /// Only show logs with these levels. Default empty (prints all levels)
    #[clap(short, long)]
    filter_levels: Option<Vec<String>>,

    /// The number of empty lines printed after formatted logs
    #[clap(short, long, default_value = "0")]
    spacing: i64,

    /// Do not attempt to print logs in color
    #[clap(short, long)]
    disable_colors: bool,

    /// Hides any log lines that are not valid json
    #[clap(long)]
    hide_non_json: bool,

    /// Prints each extra field on it's own line
    #[clap(long)]
    multiline_fields: bool,

    /// Use jql (https://github.com/yamafaktory/jql)
    /// processes extra fields using jql before display
    /// if the result of jql returns no fields, do not display the entry
    #[clap(long)]
    jql: Option<String>,

    /// Use jql (https://github.com/yamafaktory/jql)
    /// only displays lines where jql returns non empty
    #[clap(long)]
    jql_filter: Option<String>,

    timestamp_format: Option<String>,
}

pub struct LogTransformer {
    message_field: String,
    level_field: String,
    timestamp_field: String,
    excluded_fields: HashSet<String>,
    separator: String,
    hide_extra_fields: bool,
    filter_levels: HashSet<String>,
    disable_colors: bool,
    spacing: i64,
    hide_non_json: bool,
    multiline_fields: bool,
    jql: Option<String>,
    jql_filter: Option<String>,
}

impl LogTransformer {
    pub fn new(config: Config) -> LogTransformer {
        let mut excluded_fields = match (&config.exclude_fields).as_ref() {
            Some(excluded) => excluded.into_iter().cloned().collect(),
            None => HashSet::new(),
        };

        let filter_levels = match (&config.filter_levels).as_ref() {
            Some(filtered) => filtered.into_iter().cloned().collect(),
            None => HashSet::new(),
        };

        excluded_fields.insert(config.message_field_name.clone());
        excluded_fields.insert(config.level_field_name.clone());
        excluded_fields.insert(config.timestamp_field_name.clone());

        LogTransformer {
            message_field: config.message_field_name.clone(),
            level_field: config.level_field_name.clone(),
            timestamp_field: config.timestamp_field_name.clone(),
            excluded_fields,
            separator: config.separator.clone(),
            hide_extra_fields: config.hide_extra_fields,
            disable_colors: config.disable_colors,
            hide_non_json: config.hide_non_json,
            spacing: config.spacing,
            filter_levels,
            multiline_fields: config.multiline_fields,
            jql: config.jql,
            jql_filter: config.jql_filter
        }
    }

    pub fn transform_and_print(&self, line: String) -> anyhow::Result<()> {
        let json_value = serde_json::from_str::<Value>(&line);

        if let Ok(val) = json_value.as_ref() {
            let json_obj = val.as_object();

            match json_obj {
                Some(obj) => {
                    let message = obj.get(&self.message_field).unwrap().as_str().unwrap_or("???");
                    let level = obj.get(&self.level_field).unwrap().as_str().unwrap_or("???");

                    // Skip if this level isn't in the level filter list
                    if !self.filter_levels.is_empty() && !self.filter_levels.contains(level) {
                        return Ok(());
                    }

                    let time = obj.get(&self.timestamp_field).unwrap().as_f64();

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

                    col!(match level.trim().to_lowercase().as_str() {
                            "trace" | "debug" => Color::Black,
                            "info" | "notice" => Color::Blue,
                            "warning" => Color::Yellow,
                            "error" | "err" | "critical" | "crit" | "fatal" | "emerg" | "emergency" | "alert" => Color::Red,
                            _ => Color::Black
                        });

                    write!(&mut buffer, "[{}] ", level).unwrap();

                    col!(Color::Black);
                    write!(&mut buffer, "{}", message).unwrap();

                    if message.len() > MULTILINE_MESSAGE_THRESHOLD && !self.multiline_fields {
                        write!(&mut buffer, "\n")?;
                    }

                    if !self.hide_extra_fields {
                        if let Some(query) = &self.jql_filter {
                            let walked = jql::walker(val, Some(query));

                            match walked {
                                Err(_) => {
                                    return Ok(())
                                }
                                Ok(_) => {}
                            }
                        }

                        let vv = if let Some(query) = &self.jql {
                            let walked = jql::walker(val, Some(query));

                            let query_result = match walked {
                                Ok(qr) => match qr {
                                    Value::Object(_) => qr,
                                    _ => Value::Object(Map::from_iter(std::iter::once(("_jlf_inner".to_string(), qr)))),
                                },
                                Err(why) => match why.as_str() {
                                    "Empty group" => {
                                        // The jql query has resulted in no values, skip this log entry
                                        return Ok(());
                                    }
                                    _ => {
                                        if why.contains("not found on the parent element") {
                                            return Ok(());
                                        }

                                        panic!("{}", why);
                                    }
                                }
                            };

                            query_result
                        } else {
                            val.clone()
                        };

                        let extra_fields: Vec<(String, String)> = vv.as_object().unwrap().iter()
                            .filter(|(k, _)| !self.excluded_fields.contains(k.as_str()))
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

                                (k.clone(), formatted_value.to_string())
                            }).collect();

                        for (k, v) in extra_fields {
                            if self.multiline_fields {
                                write!(&mut buffer, "\n")?;
                                write!(&mut buffer, "  ")?;
                                col!(Color::Green);
                                write!(&mut buffer, "{}", k)?;

                                if v.len() > MULTILINE_FIELD_THRESHOLD || v.contains("\n") {
                                    col!(Color::Black);
                                    write!(&mut buffer, ":\n")?;
                                    col!(Color::Black);
                                    write!(&mut buffer, "{}", v)?;
                                    write!(&mut buffer, "\n")?;
                                } else {
                                    col!(Color::Black);
                                    write!(&mut buffer, "=")?;
                                    col!(Color::Black);
                                    write!(&mut buffer, "{}", v)?;
                                }
                            } else {
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
                    }


                    col!(Color::Black);
                    write!(&mut buffer, "\n")?;

                    for _ in 0..self.spacing {
                        write!(&mut buffer, "\n")?;
                    }

                    bufwtr.print(&buffer)?;
                }
                None => {
                    if !self.hide_non_json {
                        println!("{}", line);
                    }
                }
            }
        } else {
            if !self.hide_non_json {
                println!("{}", line);
            }
        }

        Ok(())
    }
}
