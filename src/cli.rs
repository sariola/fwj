use clap::Parser;
use log::LevelFilter;

/// CLI tool for processing tasks
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the data file
    #[arg(short, long, default_value = "./data/subquery-data.json")]
    pub data: String,

    /// Optional config file path
    #[arg(short, long, default_value = "config.yaml")]
    pub config: String,

    /// Verbose mode
    #[arg(short, long, default_value = "false")]
    pub verbose: bool,

    /// Set the logging level (off, error, warn, info, debug, trace)
    #[arg(short, long, default_value = "error")]
    pub log_level: LevelFilter,

    /// Path to the rubric Jinja template
    #[arg(short, long, default_value = "./rubrics/subquery-decomp.jinja")]
    pub rubric: String,

    /// Display the last result
    #[arg(long, default_value = "false")]
    pub last_result: bool,

    /// Set the concurrent batch size
    #[arg(short = 'b', long, default_value = "10")]
    pub batch_size: usize,
}

pub fn parse_args() -> Args {
    Args::parse()
}
