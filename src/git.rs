use dashmap::DashMap;
use git2::{ObjectType, Oid, Repository};
use std::collections::HashSet;
use std::path::PathBuf;

pub fn get_file_commit_history(
    repo_path: &str,
    max_history_length: i64,
) -> Result<DashMap<String, HashSet<Oid>>, git2::Error> {
    let repo = Repository::open(repo_path)?;
    let file_commits: DashMap<String, HashSet<Oid>> = DashMap::new();

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
                            .or_insert_with(HashSet::new)
                            .insert(commit_id);
                    };
                    true
                },
                None,
                Some(&mut |delta, _| {
                    if let Some(new_file) = delta.new_file().path() {
                        println!("New file: {}", new_file.display());
                        file_commits
                            .entry(new_file.to_str().unwrap().to_owned())
                            .or_insert_with(HashSet::new)
                            .insert(commit_id);
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
                        .or_insert_with(HashSet::new)
                        .insert(commit_id);
                }
                git2::TreeWalkResult::Ok
            })?;
        }
        i += 1;
        if i > max_history_length {
            break;
        }
    }
    Ok(file_commits)
}
