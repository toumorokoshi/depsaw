use super::super::bazel;
use super::super::git;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::RwLock;
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
    /// The commits that trigger this target specifically. Does not include commits
    /// that triggered dependencies.
    #[serde(skip_serializing, skip_deserializing)]
    pub commits: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    /// number of times the target is rebuilt
    pub rebuilds: usize,
    /// number of targets that depend on this target
    pub immediate_dependents: Vec<Rc<RwLock<Target>>>,
}

impl Ord for ResolvedTarget {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

impl PartialOrd for ResolvedTarget {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.score.cmp(&other.score))
    }
}

pub fn calculate_trigger_scores(
    target: &str,
    repo: &git::GitRepo,
    deps_graph: &bazel::BazelDependencyGraph,
) -> anyhow::Result<HashMap<String, ResolvedTarget>> {
    let mut commits_by_target = HashMap::new();
    let mut commits_specific_to_target = HashMap::new();
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
                    &mut commits_specific_to_target,
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
            &mut commits_specific_to_target,
            &mut score_by_target,
        )?;
    }
    let mut result = HashMap::new();
    // calculate values that were not calculatable in the first pass
    for (_, target_rw) in score_by_target.iter_mut() {
        let target = target_rw.read().unwrap();
        let total_dependents = recursively_calculate_total_dependents(&target_rw);
        let score = target.rebuilds * (total_dependents + 1);
        result.insert(
            target.name.clone(),
            ResolvedTarget {
                name: target.name.clone(),
                rebuilds: target.rebuilds,
                immediate_dependents: target.immediate_dependents.len(),
                total_dependents: total_dependents,
                score,
                commits: commits_specific_to_target
                    .get(&target.name)
                    .ok_or(anyhow!(
                        "target {} not found in commits_specific_to_target",
                        target.name
                    ))?
                    .clone(),
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
    commits_specific_to_target: &mut HashMap<String, std::collections::HashSet<String>>,
    score_by_target: &mut HashMap<String, Rc<RwLock<Target>>>,
) -> anyhow::Result<std::collections::HashSet<String>> {
    if let Some(commits) = commits_by_target.get(target_name) {
        return Ok(commits.clone());
    }
    let mut all_commits: std::collections::HashSet<String> = std::collections::HashSet::new();
    let rule = deps_graph.rules_by_label.get(target_name).ok_or(anyhow!(
        "target {} not found in dependency graph",
        target_name
    ))?;
    let target_rc = Rc::new(RwLock::new(Target {
        name: target_name.to_string(),
        rebuilds: 0,
        immediate_dependents: vec![],
    }));
    for dep_target in rule.dep_targets.iter() {
        all_commits.extend(calculate_trigger_scores_map_inner(
            dep_target,
            repo,
            deps_graph,
            commits_by_target,
            commits_specific_to_target,
            score_by_target,
        )?);
        let mut target = score_by_target.get(dep_target).unwrap().write().unwrap();
        target.immediate_dependents.push(target_rc.clone());
    }
    let mut commits_touching_files = HashSet::new();
    for source_file in rule.source_files.iter() {
        // we don't care about remote dependencies
        if source_file.starts_with("@") {
            continue;
        }
        let parts: Vec<&str> = source_file.split(':').collect();
        let relative_path = &format!("{}/{}", parts[0], parts[1])[2..];

        if let Some(file) = repo.files.get(relative_path) {
            commits_touching_files.extend(file.commit_history.iter().cloned());
        }
    }
    all_commits.extend(commits_touching_files.iter().cloned());
    let mut target = target_rc.write().unwrap();
    target.rebuilds = all_commits.len();
    score_by_target.insert(target_name.to_string(), target_rc.clone());
    commits_by_target.insert(target_name.to_string(), all_commits.clone());
    commits_specific_to_target.insert(target_name.to_string(), commits_touching_files);
    Ok(all_commits)
}

fn recursively_calculate_total_dependents(target: &Rc<RwLock<Target>>) -> usize {
    let mut visited = HashSet::new();
    inner_recursively_calculate_total_dependents(target, &mut visited);
    visited.len()
}

fn inner_recursively_calculate_total_dependents(
    target: &Rc<RwLock<Target>>,
    visited: &mut HashSet<String>,
) -> usize {
    let mut total = 1;
    for dependent in target.read().unwrap().immediate_dependents.iter() {
        if visited.contains(&dependent.read().unwrap().name) {
            continue;
        }
        visited.insert(dependent.read().unwrap().name.clone());
        total += inner_recursively_calculate_total_dependents(dependent, visited);
    }
    total
}
