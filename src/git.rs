use dashmap::DashMap;
use git2::{ObjectType, Repository};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepo {
    pub files: HashMap<String, GitFile>,
}

impl GitRepo {
    pub fn from_path(path: &str, commit_history_length: i64) -> Result<GitRepo, git2::Error> {
        let files = get_file_commit_history(path, commit_history_length)?;
        Ok(GitRepo { files })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitFile {
    pub commit_history: HashSet<String>,
}

fn get_file_commit_history(
    repo_path: &str,
    max_history_length: i64,
) -> Result<HashMap<String, GitFile>, git2::Error> {
    let repo = Repository::open(repo_path)?;
    let file_commits: DashMap<String, GitFile> = DashMap::new();

    // Get the HEAD reference
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;

    // Create a revwalk to iterate through all commits
    let mut revwalk = repo.revwalk()?;
    revwalk.push(head_commit.id())?;

    let mut i = 0;
    // Iterate through all commits
    for oid in revwalk {
        let commit_id = oid?;
        let commit = repo.find_commit(commit_id)?;

        if commit.parent_count() > 0 {
            let parent = commit.parent(0)?;
            let tree = commit.tree()?;
            let parent_tree = parent.tree()?;

            // Get the diff between this commit and its parent
            let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

            // Process each file change in the diff
            diff.foreach(
                &mut |delta, _| {
                    if let Some(new_file) = delta.new_file().path() {
                        file_commits
                            .entry(new_file.to_str().unwrap().to_owned())
                            .or_insert_with(|| GitFile {
                                commit_history: HashSet::new(),
                            })
                            .commit_history
                            .insert(commit_id.to_string());
                    };
                    true
                },
                None,
                Some(&mut |delta, _| {
                    if let Some(new_file) = delta.new_file().path() {
                        // kjprintln!("New file: {}", new_file.display());
                        file_commits
                            .entry(new_file.to_str().unwrap().to_owned())
                            .or_insert_with(|| GitFile {
                                commit_history: HashSet::new(),
                            })
                            .commit_history
                            .insert(commit_id.to_string());
                    };
                    true
                }),
                None,
            )?;
        } else {
            // For the initial commit, add all files
            let tree = commit.tree()?;
            tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
                if entry.kind() == Some(ObjectType::Blob) {
                    let path = PathBuf::from(entry.name().unwrap_or(""));
                    file_commits
                        .entry(path.to_str().unwrap().to_owned())
                        .or_insert_with(|| GitFile {
                            commit_history: HashSet::new(),
                        })
                        .commit_history
                        .insert(commit_id.to_string());
                }
                git2::TreeWalkResult::Ok
            })?;
        }
        if max_history_length != -1 {
            i += 1;
            if i > max_history_length {
                break;
            }
        }
    }
    Ok(file_commits.into_iter().collect())
}
