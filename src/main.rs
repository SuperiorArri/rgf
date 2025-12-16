use clap::Parser;
use rgf::args::Cli;

fn main() {
    let args = Cli::parse();
    println!("{:?}", args);
}
