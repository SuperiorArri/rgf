use std::{fmt, str::FromStr};

use clap::{ArgAction, Parser};

const RGF_VERSION: &str = env!("RGF_VERSION");
const GIT_DEFAULT_REMOTE: &str = "origin";

#[derive(Parser, Debug)]
#[command(
    name = "rgf",
    version = RGF_VERSION,
    about = "RGF command-line tool",
    disable_version_flag = true
)]
pub struct Cli {
    /// Initialize (repair) project to be conform with RGF model and proceed
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
    pub yes_to_all: bool,

    /// Prefix 'v' won't be used in version names (v<version>)
    #[arg(long = "no-prefix")]
    pub no_prefix: bool,

    /// Set remote name
    #[arg(long = "remote", default_value = GIT_DEFAULT_REMOTE)]
    pub remote: String,

    /// Main branch name
    #[arg(long = "main", default_value = "main")]
    pub main_branch: String,

    /// Dev branch name
    #[arg(long = "dev", default_value = "dev")]
    pub dev_branch: String,

    /// Release branch name
    #[arg(long = "release", default_value = "release")]
    pub release_branch: String,

    /// Feature branch name prefix
    #[arg(long = "feature-prefix", default_value = "feature-")]
    pub feature_branch_prefix: String,

    /// Hotfix branch name prefix
    #[arg(long = "hotfix-prefix", default_value = "hotfix-")]
    pub hotfix_branch_prefix: String,

    /// Command to generate version file, args: [additional_args...] MAJOR MINOR PATCH
    #[arg(long = "version-file-gen-cmd", id = "vfgcmd")]
    pub version_file_gen_cmd: Option<String>,

    /// Additional arg for version file gen command (can be used multiple times)
    #[arg(long = "arg", action = clap::ArgAction::Append, requires = "vfgcmd")]
    pub version_file_gen_args: Vec<String>,

    /// Run args
    #[arg(
        num_args = 0..=2,
        value_names = ["ACTION", "REF_NAME"],
        long_help =
            "ACTION     One of: feature, hotfix, pull, push, release\n\
             REF_NAME   Git reference name (branch, tag, or commit)\n"
    )]
    pub run_args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Feature,
    Hotfix,
    Pull,
    Push,
    Release,
}

impl Action {
    const fn as_str(self) -> &'static str {
        match self {
            Action::Feature => "feature",
            Action::Hotfix => "hotfix",
            Action::Pull => "pull",
            Action::Push => "push",
            Action::Release => "release",
        }
    }
}

impl FromStr for Action {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            _ if s.eq_ignore_ascii_case("feature") => Ok(Action::Feature),
            _ if s.eq_ignore_ascii_case("hotfix") => Ok(Action::Hotfix),
            _ if s.eq_ignore_ascii_case("pull") => Ok(Action::Pull),
            _ if s.eq_ignore_ascii_case("push") => Ok(Action::Push),
            _ if s.eq_ignore_ascii_case("release") => Ok(Action::Release),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
