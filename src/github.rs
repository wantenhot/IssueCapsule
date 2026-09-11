use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

static ISSUE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://github\.com/([^/]+)/([^/]+)/issues/([1-9][0-9]*)/?$")
        .expect("the GitHub issue URL regex is valid")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRef {
    pub owner: String,
    pub repository: String,
    pub number: u64,
}

impl IssueRef {
    pub fn repository_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.repository)
    }

    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }
}

#[derive(Debug, Deserialize)]
pub struct IssueInfo {
    pub title: String,
    pub body: Option<String>,
}

pub fn parse_issue_url(url: &str) -> Result<IssueRef> {
    let captures = ISSUE_URL
        .captures(url)
        .ok_or_else(|| anyhow::anyhow!("Invalid GitHub issue URL."))?;

    let number = captures[3].parse().context("Invalid GitHub issue URL.")?;

    Ok(IssueRef {
        owner: captures[1].to_owned(),
        repository: captures[2].to_owned(),
        number,
    })
}

pub fn fetch_issue(issue: &IssueRef) -> Result<IssueInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/issues/{}",
        issue.owner, issue.repository, issue.number
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to create GitHub client.")?;
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "IssueCapsule/0.2")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("Failed to fetch GitHub issue.")?;

    if !response.status().is_success() {
        bail!(
            "Failed to fetch GitHub issue (HTTP {}).",
            response.status().as_u16()
        );
    }

    response
        .json()
        .context("GitHub returned an invalid issue response.")
}

#[cfg(test)]
mod tests {
    use super::parse_issue_url;

    #[test]
    fn parses_github_issue_url() {
        let issue = parse_issue_url("https://github.com/foo/bar/issues/123").unwrap();

        assert_eq!(issue.owner, "foo");
        assert_eq!(issue.repository, "bar");
        assert_eq!(issue.number, 123);
    }

    #[test]
    fn rejects_repository_url_without_issue() {
        assert!(parse_issue_url("https://github.com/foo/bar").is_err());
    }

    #[test]
    fn rejects_non_github_issue_url() {
        assert!(parse_issue_url("https://gitlab.com/foo/bar/issues/123").is_err());
    }
}
