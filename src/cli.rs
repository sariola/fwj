use clap::{Parser, Subcommand};
use log::LevelFilter;

/// CLI tool for processing tasks
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the data file or "fetch" to download
    #[arg(short, long, default_value = "fetch")]
    pub data: String,

    /// Optional config file path
    #[arg(long, default_value = "config.yaml")]
    pub config: String,

    /// Verbose mode
    #[arg(short, long, default_value = "false")]
    pub verbose: bool,

    /// Set the logging level (off, error, warn, info, debug, trace)
    #[arg(long, default_value = "error")]
    pub log_level: LevelFilter,

    /// Path to the rubric Jinja template or "fetch" to download
    #[arg(short, long, default_value = "fetch")]
    pub rubric: String,

    /// Display the last result
    #[arg(short = 'l', long, default_value = "false")]
    pub last_result: bool,

    /// Set the concurrent batch size
    #[arg(short = 'b', long, default_value = "1")]
    pub batch_size: usize,

    /// Context size for llamafile
    #[arg(short, long, default_value = "8192")]
    pub context_size: usize,

    /// GPU layers for llamafile
    #[arg(long, default_value = "32")]
    pub gpu_layers: usize,

    /// Temperature for llamafile
    #[arg(long = "temp", default_value = "0.1")]
    pub temperature: f32,

    /// Max tokens for llamafile
    #[arg(short = 'n', long, default_value = "1000")]
    pub max_tokens: usize,

    /// Thread count for llamafile (default: number of available threads)
    #[arg(short = 't', long)]
    pub thread_count: Option<usize>,

    /// Additional llamafile arguments as key-value pairs (e.g., "key1=value1,key2=value2")
    #[arg(short = 'a', long)]
    pub llamafile_kvargs: Option<String>,

    /// Enable key-value offloading
    #[arg(long)]
    pub enable_kv_offload: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate auto-completions
    GenAutoCompletions {
        #[arg(value_enum)]
        shell: Shell,
        /// Output file path (optional)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
    Elvish,
    PowerShell,
}

pub fn parse_args() -> Args {
    Args::parse()
}
