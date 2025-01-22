use super::super::bazel::BazelDependencyGraph;
use super::super::git::GitRepo;
use super::calculate_trigger_scores;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub score: usize,
}

/// Calculates which dependencies would save the most triggers if removed.
/// Returns a map of dependency name to potential trigger reduction score.
pub fn most_unique_triggers(
    target: &str,
    repo: &GitRepo,
    deps_graph: &BazelDependencyGraph,
) -> Result<Vec<Dependency>> {
    // Get all scores for the dependency graph
    let scores_by_target = calculate_trigger_scores(target, repo, deps_graph)?;

    // Get the immediate dependencies of our target
    let target_rule = deps_graph
        .rules_by_label
        .get(target)
        .ok_or(anyhow!("Target {} not found in dependency graph", target))?;

    // Find duplicate dependencies (deps that are pulled in through multiple paths)
    let duplicate_deps = find_duplicate_deps(target, deps_graph)?;

    // Calculate potential savings for each immediate dependency
    let mut deps = Vec::new();

    for dep in &target_rule.dep_targets {
        // Skip if this is a duplicate dependency
        if duplicate_deps.contains(dep) {
            continue;
        }

        // Get all unique dependencies that would be removed if we removed this dep
        let unique_deps = get_unique_deps(dep, deps_graph, &duplicate_deps)?;

        // Calculate total score from all unique deps
        let mut commits = HashSet::new();
        for unique_dep in unique_deps {
            if let Some(dep_score) = scores_by_target.get(&unique_dep) {
                commits.extend(dep_score.commits.iter().cloned());
            }
        }

        deps.push(Dependency {
            name: dep.clone(),
            score: commits.len(),
        });
    }

    // sort by score
    deps.sort_by(|a, b| b.score.cmp(&a.score));

    Ok(deps)
}

/// Finds dependencies that are pulled in through multiple paths in the graph
fn find_duplicate_deps(target: &str, deps_graph: &BazelDependencyGraph) -> Result<HashSet<String>> {
    let mut seen = HashSet::new();
    let mut duplicates = HashSet::new();

    fn visit_deps(
        current: &str,
        deps_graph: &BazelDependencyGraph,
        seen: &mut HashSet<String>,
        duplicates: &mut HashSet<String>,
    ) -> Result<()> {
        let rule = deps_graph
            .rules_by_label
            .get(current)
            .ok_or(anyhow!("Target {} not found in dependency graph", current))?;

        for dep in &rule.dep_targets {
            if !seen.insert(dep.clone()) {
                duplicates.insert(dep.clone());
            } else {
                visit_deps(dep, deps_graph, seen, duplicates)?;
            }
        }
        Ok(())
    }

    visit_deps(target, deps_graph, &mut seen, &mut duplicates)?;
    Ok(duplicates)
}

/// Gets all unique dependencies that would be removed if we removed the given dep
fn get_unique_deps(
    dep: &str,
    deps_graph: &BazelDependencyGraph,
    duplicate_deps: &HashSet<String>,
) -> Result<HashSet<String>> {
    let mut unique_deps = HashSet::new();

    fn visit_deps(
        current: &str,
        deps_graph: &BazelDependencyGraph,
        duplicate_deps: &HashSet<String>,
        unique_deps: &mut HashSet<String>,
    ) -> Result<()> {
        if duplicate_deps.contains(current) {
            return Ok(());
        }

        unique_deps.insert(current.to_string());

        let rule = deps_graph
            .rules_by_label
            .get(current)
            .ok_or(anyhow!("Target {} not found in dependency graph", current))?;

        for dep in &rule.dep_targets {
            visit_deps(dep, deps_graph, duplicate_deps, unique_deps)?;
        }
        Ok(())
    }

    visit_deps(dep, deps_graph, duplicate_deps, &mut unique_deps)?;
    Ok(unique_deps)
}
