use super::bazel;
use super::git;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::rc::Rc;

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerScores {
    pub targets: Vec<ResolvedTarget>,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct ResolvedTarget {
    pub name: String,
    /// number of times the target is rebuilt
    pub rebuilds: usize,
    /// number of targets that depend on this target
    pub immediate_dependents: usize,
    /// score refers to how much the target is responsible for triggering
    pub total_dependents: usize,
    /// builds. it is currently rebuilds + dependents.
    pub score: usize,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct Target {
    pub name: String,
    /// number of times the target is rebuilt
    pub rebuilds: usize,
    /// number of targets that depend on this target
    pub immediate_dependents: Vec<Rc<Target>>,
    /// builds. it is currently rebuilds + dependents.
    pub score: usize,
}

impl Ord for ResolvedTarget {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rebuilds.cmp(&other.rebuilds)
    }
}

impl PartialOrd for ResolvedTarget {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.rebuilds.cmp(&other.rebuilds))
    }
}

pub fn calculate_trigger_scores_map(
    target: &str,
    repo: &git::GitRepo,
    deps_graph: &bazel::BazelDependencyGraph,
) -> Result<HashMap<String, ResolvedTarget>, Box<dyn Error>> {
    let mut commits_by_target = HashMap::new();
    let mut score_by_target = HashMap::new();
    if target.ends_with("...") {
        let prefix = target[..target.len() - 4].to_string();
        // we grab all targets from the map, in this case.
        for (t, _) in deps_graph.rules_by_label.iter() {
            if t.starts_with(&prefix) {
                calculate_trigger_scores_map_inner(
                    t,
                    repo,
                    deps_graph,
                    &mut commits_by_target,
                    &mut score_by_target,
                )?;
            }
        }
    } else {
        calculate_trigger_scores_map_inner(
            target,
            repo,
            deps_graph,
            &mut commits_by_target,
            &mut score_by_target,
        )?;
    }
    let mut result = HashMap::new();
    // calculate values that were not calculatable in the first pass
    for (_, target) in score_by_target.iter_mut() {
        let score = target.rebuilds * target.immediate_dependents.len();
        let total_dependents = recursively_calculate_total_dependents(target);
        result.insert(
            target.name.clone(),
            ResolvedTarget {
                name: target.name.clone(),
                rebuilds: target.rebuilds,
                immediate_dependents: target.immediate_dependents.len(),
                total_dependents: total_dependents,
                score,
            },
        );
    }
    Ok(result)
}

fn calculate_trigger_scores_map_inner(
    target_name: &str,
    repo: &git::GitRepo,
    deps_graph: &bazel::BazelDependencyGraph,
    commits_by_target: &mut HashMap<String, std::collections::HashSet<String>>,
    score_by_target: &mut HashMap<String, Rc<Target>>,
) -> Result<std::collections::HashSet<String>, Box<dyn Error>> {
    if let Some(commits) = commits_by_target.get(target_name) {
        return Ok(commits.clone());
    }
    let mut all_commits: std::collections::HashSet<String> = std::collections::HashSet::new();
    let rule = deps_graph.rules_by_label.get(target_name).ok_or(format!(
        "target {} not found in dependency graph",
        target_name
    ))?;
    let mut target_rc = Rc::new(Target {
        name: target_name.to_string(),
        rebuilds: 0,
        immediate_dependents: vec![],
        score: 0,
    });
    for dep_target in rule.dep_targets.iter() {
        all_commits.extend(calculate_trigger_scores_map_inner(
            dep_target,
            repo,
            deps_graph,
            commits_by_target,
            score_by_target,
        )?);
        score_by_target
            .entry(dep_target.to_string())
            .and_modify(|t| {
                Rc::get_mut(t)
                    .unwrap()
                    .immediate_dependents
                    .push(target_rc.clone())
            });
    }
    for source_file in rule.source_files.iter() {
        // we don't care about remote dependencies
        if source_file.starts_with("@") {
            continue;
        }
        let parts: Vec<&str> = source_file.split(':').collect();
        let relative_path = &format!("{}/{}", parts[0], parts[1])[2..];

        // println!("Analyzing source file: {}", source_file);
        if let Some(file) = repo.files.get(relative_path) {
            // println!("Found {} commits for {}", commits.len(), source_file);
            all_commits.extend(file.commit_history.iter().cloned());
        }
    }
    let target = Rc::get_mut(&mut target_rc).unwrap();
    target.rebuilds = all_commits.len();
    score_by_target.insert(target_name.to_string(), target_rc);
    commits_by_target.insert(target_name.to_string(), all_commits.clone());
    Ok(all_commits)
}

fn recursively_calculate_total_dependents(target: &Rc<Target>) -> usize {
    let mut visited = HashSet::new();
    inner_recursively_calculate_total_dependents(target, &mut visited);
    visited.len()
}

fn inner_recursively_calculate_total_dependents(
    target: &Rc<Target>,
    visited: &mut HashSet<String>,
) -> usize {
    let mut total = 1;
    for dependent in target.immediate_dependents.iter() {
        if visited.contains(&dependent.name) {
            continue;
        }
        visited.insert(dependent.name.clone());
        total += inner_recursively_calculate_total_dependents(dependent, visited);
    }
    total
}
