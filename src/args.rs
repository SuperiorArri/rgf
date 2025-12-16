use clap::{ArgAction, Parser, ValueEnum};
use std::path::PathBuf;

const RGF_VERSION: &str = env!("RGF_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "rgf",
    version = RGF_VERSION,
    about = "RGF command-line tool",
    disable_version_flag = true
)]
pub struct Cli {
    /// Repair (initialize) project to be conform with RGF model and proceed
    #[arg(short = 'c', long = "conform")]
    pub conform: bool,

    /// Use markers to highlight command status
    #[arg(
        long="color",
        alias="colour",
        default_value = "auto",
        num_args=0..=1
    )]
    pub color: ColorWhen,

    /// Move (stash and pop) uncommitted changes
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// Same as conform, but do not proceed
    #[arg(short = 'i', long = "init")]
    pub init: bool,

    /// Create new release branch with next major version (x.0.0)
    #[arg(short = 'm', long = "major")]
    pub major: bool,

    /// Do not run commands; only parse user options
    #[arg(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// Prepare branch for pull request and push to origin
    #[arg(short = 'r', long = "request")]
    pub request: bool,

    /// Verbose mode
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Print version number
    #[arg(short = 'V', long = "version", action = ArgAction::Version)]
    pub version: Option<bool>,

    /// Display what to do on current branch
    #[arg(short = 'w', long = "what-now")]
    pub what_now: bool,

    /// Assume yes for all questions
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Main branch name
    #[arg(long = "main", default_value = "main")]
    pub main_branch: String,

    /// Dev branch name
    #[arg(long = "dev", default_value = "dev")]
    pub dev_branch: String,

    /// Feature branch name prefix
    #[arg(long = "feature-prefix", default_value = "feature-")]
    pub feature_branch_prefix: String,

    /// Hotfix branch name prefix
    #[arg(long = "hotfix-prefix", default_value = "hotfix-")]
    pub hotfix_branch_prefix: String,

    /// Path to version update script (called with version string as an argument)
    #[arg(long = "version-update-script")]
    pub version_update_script: Option<PathBuf>,

    /// Optional positional keyword
    pub keyword: Option<String>,

    /// Optional positional name
    pub name: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ColorWhen {
    #[value(name = "always")]
    Always,
    #[value(name = "never")]
    Never,
    #[value(name = "auto")]
    Auto,
}
