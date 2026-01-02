use crate::git::GitResult;
use clap::Parser;
use git::GitVersion;
use regex::Regex;
use std::{
    fmt::Display,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use thiserror::Error;
use users::{get_current_uid, get_user_by_uid};

pub mod args;
pub mod git;

const MIN_GIT_VERSION: GitVersion = GitVersion(1, 8, 0);
const CHANGELOG_FILE: &str = "CHANGELOG.md";
const VERSION_FILE: &str = "VERSION";

const CHANGELOG_HEADER: &str = r#"# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/)."#;

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RepoVersion(pub u32, pub u32, pub u32);

impl Display for RepoVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

impl RepoVersion {
    pub fn inc_major(&self) -> RepoVersion {
        RepoVersion(self.0 + 1, 0, 0)
    }

    pub fn inc_minor(&self) -> RepoVersion {
        RepoVersion(self.0, self.1 + 1, 0)
    }

    pub fn inc_patch(&self) -> RepoVersion {
        RepoVersion(self.0, self.1, self.2 + 1)
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("invalid action '{0}'")]
    InvalidAction(String),

    #[error("minimum Git version required is {required}, current is {actual}")]
    MinGitVersionRequired {
        required: GitVersion,
        actual: GitVersion,
    },

    #[error("Git repository does not exist")]
    GitRepoMissing,

    #[error("Git repository without commits")]
    GitNoCommits,

    #[error("Git stage area is not empty")]
    GitStageAreaNotEmpty,

    #[error("changelog file already exists, please remove it before proceeding")]
    ChangelogAlreadyExists,

    #[error("missing remote '{0}'")]
    GitMissingRemote(String),

    #[error("missing branch '{0}'")]
    GitMissingBranch(String),

    #[error("missing version tag on branch '{0}'")]
    GitMissingVersionTagOnBranch(String),

    #[error("branch '{0}' not merged with branch '{1}'")]
    BranchesNotMerged(String, String),

    #[error("current branch '{0}' not recognized")]
    BranchNotRecognized(String),

    #[error("invalid branch name format '{0}'")]
    GitInvalidBranchNameFormat(String),

    #[error("file '{0}' IO error: {1}")]
    FileIo(PathBuf, #[source] std::io::Error),

    #[error("file '{0}' missing")]
    FileMissing(PathBuf),

    #[error("failed to execute version gen command ({0}): {1}")]
    VersionGenCmdError(String, #[source] std::io::Error),

    #[error("changelog file missing expected header")]
    ChangelogFileMissingHeader,

    #[error(
        "auto branch naming failed: could not get username, please specify branch name in cli argument"
    )]
    CouldNotGetUsername,

    // #[error("IO error: {0}")]
    // Io(#[from] std::io::Error),
    #[error(transparent)]
    Git(#[from] git::GitError),
}

pub enum BranchKind {
    DetachedHead,
    Main,
    Dev,
    Stable,
    Feature,
    Hotfix,
    Release,
}

pub struct App {
    args: args::Cli,
    repo_version: Option<RepoVersion>,
    arg_action: Option<args::Action>,
    arg_ref_name: Option<String>,
    default_repo_version: RepoVersion,
}

pub fn get_args() -> args::Cli {
    args::Cli::parse()
}

impl App {
    pub fn new(args: args::Cli) -> Self {
        Self {
            args,
            repo_version: None,
            arg_action: None,
            arg_ref_name: None,
            default_repo_version: Default::default(),
        }
    }

    pub fn run(&mut self) -> AppResult<()> {
        self.parse_run_args()?;

        let git_version = git::version()?;
        check_min_git_version(&git_version)?;

        if self.args.verbose {
            println!("Git version: {:?}", git_version);
        }

        if self.args.init {
            self.init()?;
            return Ok(());
        }

        self.do_checks()?;
        self.load_repo_version();

        if self.args.verbose {
            if let Some(v) = self.repo_version.as_ref() {
                println!("Repo version: {v}");
            } else {
                println!("No repo version");
            }
        }

        let curr_branch = git::current_branch()?;
        self.execute(&curr_branch)?;

        Ok(())
    }

    fn repo_version(&self) -> &RepoVersion {
        self.repo_version
            .as_ref()
            .unwrap_or(&self.default_repo_version)
    }

    fn parse_run_args(&mut self) -> AppResult<()> {
        let args = &self.args.run_args;
        if args.len() == 2 {
            let action = args::Action::from_str(&args[0])
                .map_err(|_| AppError::InvalidAction(String::new()))?;
            self.arg_action = Some(action);
            self.arg_ref_name = Some(args[1].to_owned());
        } else if args.len() == 1 {
            if let Ok(action) = args::Action::from_str(&args[0]) {
                self.arg_action = Some(action);
            } else {
                self.arg_ref_name = Some(args[0].to_owned());
            }
        }
        Ok(())
    }

    fn load_repo_version(&mut self) {
        if let Ok(tag) = git::last_tag_on_branch(&self.args.main_branch) {
            self.repo_version = self.parse_version_tag(&tag);
        }
    }

    fn init(&mut self) -> AppResult<()> {
        self.init_git_repo()?;
        self.init_main_branch()?;
        self.load_repo_version();
        self.init_files()?;
        self.init_tag()?;
        self.init_dev_branch()?;
        Ok(())
    }

    fn init_git_repo(&self) -> AppResult<()> {
        if !git::repo_exists() {
            if self.args.verbose {
                println!("Initializing git repository...");
            }

            git::init(&self.args.main_branch)?;

            if self.args.verbose {
                println!("Done.");
            }
        }

        if git::has_uncommitted_tracked_changes()? {
            if !git::has_commits() {
                if self.args.verbose {
                    println!("Creating initial commit...");
                }

                git::switch_or_create_branch(&self.args.main_branch)?;
                git::create_initial_commit("Commit initial files")?;

                if self.args.verbose {
                    println!("Done.");
                }
            } else {
                return Err(AppError::GitStageAreaNotEmpty);
            }
        }

        Ok(())
    }

    fn init_main_branch(&self) -> AppResult<()> {
        if !git::branch_exists(&self.args.main_branch) {
            if git::has_commits() {
                if git_remote_branch_exists(&self.args.remote, &self.args.main_branch) {
                    let target = format!("{}/{}", self.args.remote, self.args.main_branch);
                    git::create_branch_from(&self.args.main_branch, Some(&target))?;
                } else {
                    git::create_branch_from(&self.args.main_branch, None)?;
                }
            } else {
                git::switch_or_create_branch(&self.args.main_branch)?;
            }
        }

        Ok(())
    }

    fn init_files(&self) -> AppResult<()> {
        let version_file_exists = file_exists(VERSION_FILE)?;
        self.update_version_file()?;
        git::add(Path::new(VERSION_FILE))?;

        if git::has_uncommitted_tracked_changes()? {
            let kw = if version_file_exists {
                "Update"
            } else {
                "Initialize "
            };
            git::commit(&format!("{kw} '{VERSION_FILE}' file"))?;
        }

        let changelog_path = Path::new(CHANGELOG_FILE);
        if !file_exists(changelog_path)? {
            file_write(changelog_path, CHANGELOG_HEADER)?;
            git::add(changelog_path)?;
            git::commit(&format!("Initialize '{CHANGELOG_FILE}' file"))?;
        } else if !has_file_changelog_header(changelog_path)? {
            prepend_changelog_header(changelog_path)?;
            git::add(changelog_path)?;
            git::commit(&format!("Add default header to '{CHANGELOG_FILE}' file"))?;
        }

        Ok(())
    }

    fn init_tag(&self) -> AppResult<()> {
        if self.version_tag_at(&self.args.main_branch)?.is_none() {
            let new_ver = self
                .repo_version
                .as_ref()
                .map(|v| v.inc_minor())
                .unwrap_or_else(|| self.default_repo_version.clone());
            let tag = self.to_version_tag(&new_ver);
            git::add_annotated_tag(&tag, &tag)?;
        }

        Ok(())
    }

    fn init_dev_branch(&self) -> AppResult<()> {
        if !git::branch_exists(&self.args.dev_branch) {
            git::create_branch_from(&self.args.main_branch, Some(&self.args.main_branch))?;

            if git_remote_branch_exists(&self.args.remote, &self.args.dev_branch) {
                let target = format!("{}/{}", self.args.remote, self.args.dev_branch);
                git::create_branch_from(&self.args.dev_branch, Some(&target))?;
            } else {
                git::create_branch_from(&self.args.dev_branch, Some(&self.args.main_branch))?;
            }
        } else {
            git::switch_or_create_branch(&self.args.dev_branch)?;
        }

        if let Some(hash) = self.dev_missing_main_commit_hash()? {
            git_merge_new_commit_noff(&hash, &self.args.dev_branch)?;
        }

        Ok(())
    }

    fn do_checks(&self) -> AppResult<()> {
        if !git::repo_exists() {
            return Err(AppError::GitRepoMissing);
        }

        if !git::branch_exists(&self.args.main_branch) {
            return Err(AppError::GitMissingBranch(self.args.main_branch.clone()));
        }

        if !file_exists(CHANGELOG_FILE)? {
            return Err(AppError::FileMissing(PathBuf::from(CHANGELOG_FILE)));
        }

        if !file_exists(VERSION_FILE)? {
            return Err(AppError::FileMissing(PathBuf::from(VERSION_FILE)));
        }

        if self.version_tag_at(&self.args.main_branch)?.is_none() {
            return Err(AppError::GitMissingVersionTagOnBranch(
                self.args.main_branch.clone(),
            ));
        }

        if !git::branch_exists(&self.args.dev_branch) {
            return Err(AppError::GitMissingBranch(self.args.dev_branch.clone()));
        }

        if self.dev_missing_main_commit_hash()?.is_some() {
            return Err(AppError::BranchesNotMerged(
                self.args.main_branch.to_owned(),
                self.args.dev_branch.to_owned(),
            ));
        }

        if git::has_uncommitted_tracked_changes()? {
            return Err(AppError::GitStageAreaNotEmpty);
        }

        if !has_file_changelog_header(Path::new(CHANGELOG_FILE))? {
            return Err(AppError::ChangelogFileMissingHeader);
        }

        if let Some(arg_name) = self.arg_ref_name.as_ref()
            && !git::check_head_format(arg_name)
        {
            return Err(AppError::GitInvalidBranchNameFormat(arg_name.clone()));
        }

        Ok(())
    }

    fn execute(&self, curr_branch: &str) -> AppResult<()> {
        if self.arg_action.is_none() && self.arg_ref_name.is_none() {
            match self.classify_branch(curr_branch)? {
                BranchKind::DetachedHead | BranchKind::Main | BranchKind::Stable => {
                    let new_branch = format!("{}-{}", self.args.hotfix_branch_prefix, username()?);
                    let tag_prefix = self.to_version_tag_major_minor(self.repo_version());
                    self.hotfix(&new_branch, &tag_prefix)?;
                }
                BranchKind::Dev => {
                    let new_branch = format!("{}-{}", self.args.feature_branch_prefix, username()?);
                    self.feature(&new_branch)?;
                }
                BranchKind::Feature => self.merge_feature(curr_branch)?,
                BranchKind::Release => self.merge_release()?,
                BranchKind::Hotfix => self.merge_hotfix(curr_branch)?,
            }
            return Ok(());
        }

        if self.arg_ref_name.is_none()
            && let Some(arg_action) = self.arg_action
        {
            match arg_action {
                args::Action::Hotfix => {
                    let tag_prefix = self.to_version_tag_major_minor(self.repo_version());
                    self.hotfix(curr_branch, &tag_prefix)?;
                },
                args::Action::Release => {
                    if curr_branch == self.args.release_branch {
                        self.release_release()?;
                    } else {
                        self.release()?;
                    }
                }
                args::Action::Feature => self.feature(curr_branch)?,
                args::Action::Pull => self.pull()?,
                args::Action::Push => self.push()?,
            }
            return Ok(());
        }

        let arg_action = if let Some(arg_action) = self.arg_action {
            arg_action
        } else {
            match self.classify_branch(curr_branch)? {
                BranchKind::DetachedHead
                | BranchKind::Main
                | BranchKind::Stable
                | BranchKind::Release
                | BranchKind::Hotfix => args::Action::Hotfix,
                BranchKind::Dev | BranchKind::Feature => args::Action::Feature,
            }
        };

        match arg_action {
            args::Action::Hotfix => {
                let new_branch = format!("{}-{}", self.args.hotfix_branch_prefix, username()?);
                let tag_prefix = "";
                self.hotfix(&new_branch, tag_prefix)?;
            },
            args::Action::Feature => todo!(),
            _ => todo!(),
        }

        Ok(())
    }

    fn merge_hotfix(&self, branch_name: &str) -> AppResult<()> {
        Ok(())
    }

    fn merge_feature(&self, branch_name: &str) -> AppResult<()> {
        Ok(())
    }

    fn merge_release(&self) -> AppResult<()> {
        Ok(())
    }

    fn hotfix(&self, branch_name: &str, tag_prefix: &str) -> AppResult<()> {
        Ok(())
    }

    fn feature(&self, branch_name: &str) -> AppResult<()> {
        Ok(())
    }

    fn release(&self) -> AppResult<()> {
        Ok(())
    }

    fn release_release(&self) -> AppResult<()> {
        Ok(())
    }

    fn pull(&self) -> AppResult<()> {
        Ok(())
    }

    fn push(&self) -> AppResult<()> {
        Ok(())
    }

    fn update_version_file(&self) -> AppResult<()> {
        let content = if self.args.version_file_gen_cmd.is_some() {
            format!("{}\n", self.run_version_file_gen_cmd()?.trim())
        } else {
            format!("{}\n", self.repo_version())
        };
        file_write(VERSION_FILE, content)?;
        Ok(())
    }

    fn run_version_file_gen_cmd(&self) -> AppResult<String> {
        let cmd = self
            .args
            .version_file_gen_cmd
            .as_ref()
            .expect("Version file gen cmd missing. This is a logical bug.");

        let repo_version = self.repo_version();
        let stdout = std::process::Command::new(cmd)
            .args(self.args.version_file_gen_args.iter())
            .args([
                repo_version.0.to_string(),
                repo_version.1.to_string(),
                repo_version.2.to_string(),
            ])
            .output()
            .map_err(|e| AppError::VersionGenCmdError(cmd.to_owned(), e))?
            .stdout;

        let result = String::from_utf8_lossy(&stdout);
        Ok(result.into())
    }

    fn main_to_dev_last_commit_diff(&self) -> AppResult<Option<String>> {
        let out = git::run(["cherry", &self.args.dev_branch, &self.args.main_branch])?;
        let hash = out
            .lines()
            .last()
            .and_then(|line| line.split_whitespace().nth(1))
            .map(|s| s.to_string());
        Ok(hash)
    }

    fn dev_missing_main_commit_hash(&self) -> AppResult<Option<String>> {
        let hash = self.main_to_dev_last_commit_diff()?;
        if let Some(hash) = hash
            && !git::branch_contains(&hash)?.contains(&self.args.dev_branch)
        {
            return Ok(Some(hash));
        }
        Ok(None)
    }

    fn classify_branch(&self, branch_name: &str) -> AppResult<BranchKind> {
        let prefix = branch_name.split('-').next().unwrap_or(branch_name);

        if prefix == "HEAD" {
            return Ok(BranchKind::DetachedHead);
        }

        if prefix == self.args.main_branch {
            return Ok(BranchKind::Main);
        }

        if prefix == self.args.dev_branch {
            return Ok(BranchKind::Dev);
        }

        if prefix == self.args.release_branch {
            return Ok(BranchKind::Release);
        }

        if prefix.starts_with(&self.args.feature_branch_prefix) {
            return Ok(BranchKind::Feature);
        }

        if prefix.starts_with(&self.args.hotfix_branch_prefix) {
            return Ok(BranchKind::Hotfix);
        }

        if self.is_version_tag(prefix) {
            return Ok(BranchKind::Stable);
        }

        Err(AppError::BranchNotRecognized(branch_name.to_owned()))
    }

    fn is_version_tag(&self, tag: &str) -> bool {
        self.tag_regex().is_match(tag)
    }

    fn parse_version_tag(&self, tag: &str) -> Option<RepoVersion> {
        let re = self.tag_regex();
        let caps = re.captures(tag)?;

        Some(RepoVersion(
            caps[1].parse().ok()?,
            caps[2].parse().ok()?,
            caps[3].parse().ok()?,
        ))
    }

    fn to_version_tag(&self, version: &RepoVersion) -> String {
        let ver_prefix = if self.args.no_prefix { "" } else { "v" };
        format!("{ver_prefix}{version}")
    }

    fn to_version_tag_major_minor(&self, version: &RepoVersion) -> String {
        let ver_prefix = if self.args.no_prefix { "" } else { "v" };
        format!("{ver_prefix}{}.{}", version.0, version.1)
    }

    fn tag_regex(&self) -> Regex {
        let ver_prefix = if self.args.no_prefix { "" } else { "v" };
        let pattern = format!(r"^{}\d+\.\d+$", regex::escape(ver_prefix));
        Regex::new(&pattern).unwrap()
    }

    fn version_tag_at(&self, place: &str) -> AppResult<Option<RepoVersion>> {
        Ok(git::tags_at(place)?.lines().find_map(|x| self.parse_version_tag(x)))
    }

    fn user_confirm(&self, prompt: &str) -> bool {
        if self.args.yes_to_all {
            return true;
        }

        loop {
            print!("{} [y/n]: ", prompt);
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => return true,
                "n" | "no" => return false,
                _ => println!("Please enter y/yes or n/no."),
            }
        }
    }
}

fn file_exists<P>(path: P) -> AppResult<bool>
where
    P: AsRef<Path>,
{
    let path_ref = path.as_ref();
    fs::exists(path_ref).map_err(|e| AppError::FileIo(path_ref.to_owned(), e))
}

fn file_write<P, C>(path: P, contents: C) -> AppResult<()>
where
    P: AsRef<Path>,
    C: AsRef<[u8]>,
{
    let path_ref = path.as_ref();
    fs::write(path_ref, contents).map_err(|e| AppError::FileIo(path_ref.to_owned(), e))
}

fn file_read_to_string<P>(path: P) -> AppResult<String>
where
    P: AsRef<Path>,
{
    let path_ref = path.as_ref();
    fs::read_to_string(path_ref).map_err(|e| AppError::FileIo(path_ref.to_owned(), e))
}

fn file_open<P>(path: P) -> AppResult<fs::File>
where
    P: AsRef<Path>,
{
    let path_ref = path.as_ref();
    fs::File::open(path_ref).map_err(|e| AppError::FileIo(path_ref.to_owned(), e))
}

fn check_min_git_version(version: &GitVersion) -> AppResult<()> {
    if version < &MIN_GIT_VERSION {
        return Err(AppError::MinGitVersionRequired {
            required: MIN_GIT_VERSION.clone(),
            actual: version.clone(),
        });
    }

    Ok(())
}

fn git_remote_branch_exists(remote: &str, branch: &str) -> bool {
    // This can fail in case of no network access
    if let Ok(res) = git::remote_branch_exists(remote, branch) {
        return res;
    }

    git::remote_branch_exists_local(remote, branch)
}

fn git_branch_exists(remote: &str, branch: &str) -> bool {
    git::branch_exists(branch) || git_remote_branch_exists(remote, branch)
}

fn next_avail_branch_name(git_remote: &str, branch_name: &str) -> String {
    if !git_branch_exists(git_remote, branch_name) {
        branch_name.to_owned()
    } else {
        let (name, num) = parse_branch_name(branch_name);
        format!("{name}-{}", num + 1)
    }
}

// parse branch name in format branch-name-<number>, if number missing, use 1
fn parse_branch_name(s: &str) -> (&str, u32) {
    match s.rsplit_once('-') {
        Some((name, num)) => {
            let n = num.parse::<u32>().unwrap_or(1);
            (name, n)
        }
        None => (s, 1),
    }
}

fn username() -> AppResult<String> {
    if let Some(user) = get_user_by_uid(get_current_uid()) {
        Ok(user.name().to_string_lossy().to_string())
    } else {
        Err(AppError::CouldNotGetUsername)
    }
}

fn git_merge_new_commit_noff(from: &str, into_branch: &str) -> GitResult<()> {
    println!("Merging '{from}' into branch '{into_branch}'...");
    let orig_branch = git::current_branch()?;
    git::switch_or_create_branch(into_branch)?;
    git::merge_new_commit_noff(from)?;
    git::switch_or_create_branch(&orig_branch)?;
    println!("Done.");
    Ok(())
}

fn has_file_changelog_header(path: &Path) -> AppResult<bool> {
    let mut file = file_open(path)?;

    let mut buf = vec![0u8; CHANGELOG_HEADER.len()];
    let n = file
        .read(&mut buf)
        .map_err(|e| AppError::FileIo(path.to_owned(), e))?;

    if n < CHANGELOG_HEADER.len() {
        return Ok(false);
    }

    Ok(buf == CHANGELOG_HEADER.as_bytes())
}

fn prepend_changelog_header(path: &Path) -> AppResult<()> {
    let content = file_read_to_string(path)?;
    file_write(path, format!("{CHANGELOG_HEADER}\n{content}"))?;
    Ok(())
}
