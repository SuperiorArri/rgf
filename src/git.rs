use std::{ffi::OsStr, fmt::Display, path::Path};
use thiserror::Error;

pub type GitResult<T> = std::result::Result<T, GitError>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion(pub u32, pub u32, pub u32);

impl Display for GitVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git command failed: {stderr}")]
    CommandFailed { stderr: String, status: i32 },

    #[error("failed to execute Git command: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse Git version '{version_str}': {reason}")]
    InvalidVersionString { version_str: String, reason: String },
}

pub fn run<I, S>(args: I) -> GitResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = std::process::Command::new("git");
    cmd.args(args);

    println!("{:?}", cmd);

    let out = cmd.output()?;

    if !out.status.success() {
        let mut err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            err = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
        return Err(GitError::CommandFailed {
            status: out.status.code().unwrap_or(-1),
            stderr: err,
        });
    }

    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn run_and_check<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = std::process::Command::new("git");
    cmd.args(args);

    println!("{:?}", cmd);

    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

pub fn parse_version(s: &str) -> GitResult<GitVersion> {
    let part = s
        .split_whitespace()
        .flat_map(|w| w.split(|c: char| !c.is_ascii_digit() && c != '.'))
        .find(|p| p.chars().any(|c| c.is_ascii_digit()))
        .ok_or_else(|| GitError::InvalidVersionString {
            version_str: s.into(),
            reason: "invalid format".into(),
        })?;

    let mut it = part.split('.');

    let major = it
        .next()
        .ok_or_else(|| GitError::InvalidVersionString {
            version_str: s.into(),
            reason: "missing major version".into(),
        })?
        .parse()
        .map_err(|_| GitError::InvalidVersionString {
            version_str: s.into(),
            reason: "major version must be an unsigned integer".into(),
        })?;

    let minor = it
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| GitError::InvalidVersionString {
            version_str: s.into(),
            reason: "minor version must be an unsigned integer".into(),
        })?;

    let patch = it
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| GitError::InvalidVersionString {
            version_str: s.into(),
            reason: "patchlevel must be an unsigned integer".into(),
        })?;

    Ok(GitVersion(major, minor, patch))
}

pub fn version_string() -> GitResult<String> {
    run(["--version"])
}

pub fn version() -> GitResult<GitVersion> {
    let vstr = version_string()?;
    parse_version(&vstr)
}

pub fn branch_exists(branch: &str) -> bool {
    let br = format!("refs/heads/{branch}");
    run_and_check(["show-ref", "--verify", "--quiet", &br])
}

pub fn remote_branch_exists(remote: &str, branch: &str) -> GitResult<bool> {
    let out = run(["ls-remote", "--heads", remote, branch])?;
    Ok(!out.is_empty())
}

pub fn remote_branch_exists_local(remote: &str, branch: &str) -> bool {
    let r = format!("refs/remotes/{remote}/{branch}");
    run_and_check(["show-ref", "--verify", "--quiet", &r])
}

pub fn has_uncommitted_changes() -> Result<bool, GitError> {
    let out = run(["status", "--porcelain"])?;
    Ok(!out.is_empty())
}

pub fn has_uncommitted_tracked_changes() -> Result<bool, GitError> {
    let out = run(["status", "--porcelain", "--untracked-files=no"])?;
    Ok(!out.is_empty())
}

pub fn init(initial_branch: &str) -> GitResult<()> {
    if run(["init", "--initial-branch", initial_branch]).is_ok() {
        return Ok(());
    }
    run(["init"])?;
    run(["checkout", "-B", initial_branch])?;
    Ok(())
}

pub fn create_branch_from(branch: &str, from: Option<&str>) -> GitResult<()> {
    let target = from.unwrap_or("HEAD").to_string();
    run(["branch", branch, &target])?;
    Ok(())
}

pub fn switch_or_create_branch(branch: &str) -> GitResult<()> {
    if run(["switch", "-C", branch]).is_ok() {
        return Ok(());
    }
    run(["checkout", "-B", branch])?;
    Ok(())
}

pub fn commit(msg: &str) -> GitResult<()> {
    run(["commit", "-m", msg])?;
    Ok(())
}

pub fn add_all() -> GitResult<()> {
    run(["add", "-A"])?;
    Ok(())
}

pub fn add(path: &Path) -> GitResult<()> {
    run([OsStr::new("add"), path.as_os_str()])?;
    Ok(())
}

pub fn create_initial_commit(msg: &str) -> GitResult<()> {
    add_all()?;
    commit(msg)?;
    Ok(())
}

pub fn has_commits() -> bool {
    run_and_check(["rev-parse", "--verify", "HEAD"])
}

pub fn current_branch() -> GitResult<String> {
    run(["rev-parse", "--abbrev-ref", "HEAD"])
}

pub fn remote_exists(remote: &str) -> bool {
    run_and_check(["remote", "get-url", remote])
}

pub fn repo_exists() -> bool {
    run_and_check(["rev-parse", "--git-dir"])
}

pub fn tag_exists(tag: &str) -> bool {
    let tag = format!("refs/tags/{tag}");
    run_and_check(["show-ref", "--verify", "--quiet", &tag])
}

pub fn tag_at(tag: &str, place: &str) -> GitResult<bool> {
    let out = run(["tag", "--points-at", place])?;
    Ok(out.lines().any(|t| t == tag))
}

pub fn tags_at(place: &str) -> GitResult<String> {
    run(["tag", "--points-at", place])
}

pub fn last_tag_on_branch(branch: &str) -> GitResult<String> {
    run(["describe", "--tags", "--abbrev=0", branch])
}

pub fn add_annotated_tag(tag: &str, message: &str) -> GitResult<()> {
    run(["tag", "-a", tag, "-m", message])?;
    Ok(())
}

pub fn add_tag(tag: &str) -> GitResult<()> {
    run(["tag", "-a", tag])?;
    Ok(())
}

pub fn branch_contains(commit_hash: &str) -> GitResult<String> {
    run(["branch", "--contains", commit_hash])
}

pub fn merge_new_commit_noff(from: &str) -> GitResult<()> {
    run(["merge", "--no-ff", "--commit", from])?;
    Ok(())
}

pub fn check_ref_format(ref_name: &str) -> bool {
    run_and_check(["check-ref-format", ref_name])
}

pub fn check_head_format(head_name: &str) -> bool {
    let ref_name = format!("refs/heads/{head_name}");
    check_ref_format(&ref_name)
}

#[cfg(test)]
mod tests {
    use crate::git::GitVersion;

    use super::{GitError, parse_version};

    #[test]
    fn parses_standard_git_version() {
        assert_eq!(
            parse_version("git version 2.43.0").unwrap(),
            GitVersion(2, 43, 0)
        );
    }

    #[test]
    fn parses_windows_git_version() {
        assert_eq!(
            parse_version("git version 2.34.1.windows.1").unwrap(),
            GitVersion(2, 34, 1)
        );
    }

    #[test]
    fn parses_short_version() {
        assert_eq!(
            parse_version("git version 1.8").unwrap(),
            GitVersion(1, 8, 0)
        );
    }

    #[test]
    fn parses_version_with_extra_text() {
        assert_eq!(
            parse_version("git version foo 3.1.4 bar").unwrap(),
            GitVersion(3, 1, 4)
        );
    }

    #[test]
    fn returns_err_on_no_version() {
        let err = parse_version("git version foo").unwrap_err();
        match err {
            GitError::InvalidVersionString { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn returns_err_on_empty_string() {
        let err = parse_version("").unwrap_err();
        match err {
            GitError::InvalidVersionString { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn ignores_leading_non_digits() {
        assert_eq!(
            parse_version("version v2.10.7").unwrap(),
            GitVersion(2, 10, 7)
        );
    }
}
