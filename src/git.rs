use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepo {
    pub files: HashMap<String, GitFile>,
}

impl GitRepo {
    pub fn from_path(path: &str, since: Option<String>) -> Result<GitRepo, Box<dyn Error>> {
        let files = get_file_commit_history(path, since)?;
        Ok(GitRepo { files })
    }

    pub fn from_file(path: &str) -> Result<GitRepo, Box<dyn Error>> {
        let file = File::open(path)?;
        let repo: GitRepo = serde_json::from_reader(file)?;
        Ok(repo)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitFile {
    pub commit_history: HashSet<String>,
}

fn get_file_commit_history(
    repo_path: &str,
    since: Option<String>,
) -> Result<HashMap<String, GitFile>, Box<dyn Error>> {
    let mut file_commits: HashMap<String, GitFile> = HashMap::new();

    // Build command args, conditionally adding --since
    let mut args: Vec<String> = vec![
        "log".to_string(),
        "--format=%n%H".to_string(),
        "--name-only".to_string(),
    ];
    if let Some(since_date) = since {
        let arg = format!("--since={}", since_date);
        args.push(arg);
    }

    // Run git log command
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let output_str = String::from_utf8(output.stdout)?;
    let mut lines = output_str.lines();
    lines.next();

    while let Some(commit_hash) = lines.next() {
        debug!("processing commit_hash: {}", commit_hash);
        lines.next(); // Skip the empty line
                      // Collect all files until we hit an empty line
        while let Some(file_path) = lines.next() {
            if file_path.is_empty() {
                break;
            }
            file_commits
                .entry(file_path.to_string())
                .or_insert_with(|| GitFile {
                    commit_history: HashSet::new(),
                })
                .commit_history
                .insert(commit_hash.to_string());
        }
    }

    Ok(file_commits)
}
