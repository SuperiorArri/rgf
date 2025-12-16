use clap::Parser;

pub mod args;

pub fn get_args() -> args::Cli {
    let mut cli = args::Cli::parse();
    if cli.init {
        cli.conform = true;
    }
    cli
}
