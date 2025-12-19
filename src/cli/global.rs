use clap::Args;

/// Global arguments that apply to all subcommands
#[derive(Args)]
pub struct GlobalArgs {
    /// Output raw binary data
    #[arg(short = 'r', long, global = true)]
    pub raw: bool,

    /// Suppress informational notices
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Maximum input size in bytes (0 = unlimited)
    #[arg(long, global = true, default_value = "104857600")]
    pub max_size: usize,

    /// Process files exceeding --max-size limit
    #[arg(long, global = true)]
    pub force: bool,
}
