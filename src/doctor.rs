use std::time::Duration;

use anyhow::{Result, bail};

pub fn run() -> Result<()> {
    println!("IssueCapsule Doctor\n");

    let git = crate::git::is_available();
    let docker = crate::docker::is_available();
    let daemon = docker && crate::docker::daemon_available();
    let github = github_available();

    check("Git", git);
    check("Docker", docker);
    check("Docker daemon", daemon);
    check("GitHub", github);

    if git && docker && daemon && github {
        println!("\nReady.");
        Ok(())
    } else {
        bail!("System is not ready.")
    }
}

fn check(name: &str, available: bool) {
    let marker = if available { "✓" } else { "✗" };
    println!("{marker} {name}");
}

fn github_available() -> bool {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .and_then(|client| {
            client
                .get("https://github.com")
                .header(reqwest::header::USER_AGENT, "IssueCapsule/0.1")
                .send()
        })
        .is_ok_and(|response| response.status().is_success())
}
