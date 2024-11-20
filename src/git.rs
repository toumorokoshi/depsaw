use git2::{Commit, ObjectType, Oid, Repository};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub fn get_file_commit_history(
    repo_path: &str,
) -> Result<HashMap<PathBuf, HashSet<Oid>>, git2::Error> {
    let repo = Repository::open(repo_path)?;
    let mut file_commits: HashMap<PathBuf, HashSet<Oid>> = HashMap::new();

    // Get the HEAD reference
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;

    // Create a revwalk to iterate through all commits
    let mut revwalk = repo.revwalk()?;
    revwalk.push(head_commit.id())?;

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
                        let path = PathBuf::from(new_file);
                        file_commits
                            .entry(path)
                            .or_insert_with(HashSet::new)
                            .insert(commit_id);
                    }
                    true
                },
                None,
                None,
                None,
            )?;
        } else {
            // For the initial commit, add all files
            let tree = commit.tree()?;
            tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
                if entry.kind() == Some(ObjectType::Blob) {
                    let path = PathBuf::from(entry.name().unwrap_or(""));
                    file_commits
                        .entry(path)
                        .or_insert_with(HashSet::new)
                        .insert(commit_id);
                }
                git2::TreeWalkResult::Ok
            })?;
        }
    }

    Ok(file_commits)
}
