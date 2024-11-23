use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::process::Command;

mod bazel;
mod git;
use log::{error, info};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Analyzes and identifies removable Bazel dependencies"
)]
struct Args {
    /// The subcommand to run
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Analyze removable dependencies
    Analyze {
        /// The target to analyze
        target: String,

        /// Test targets to verify against
        #[arg(long, required = true)]
        test: Vec<String>,
    },
    /// Find targets that trigger core dumps
    TriggerScores {
        /// Path to the workspace root
        workspace_path: String,

        /// The target to analyze
        target: String,

        /// The maximum number of commit history to consider
        #[arg(long, default_value = "100")]
        max_history_length: i64,

        /// Path to the dependencies file
        #[arg(long)]
        deps_file: Option<String>,
    },
    /// Analyze git repository data, outputting a JSON file
    AnalyzeGitRepo {
        /// Path to the workspace root
        workspace_path: String,

        /// Path to output JSON file
        #[arg(long)]
        output: String,

        /// The maximum number of commit history to consider
        #[arg(long, default_value = "-1")]
        max_history_length: i64,
    },
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Commands::Analyze { target, test } => {
            info!("Analyzing target: {}", target);
            info!("Test targets:");
            for test_target in &test {
                info!("  {}", test_target);
            }

            // Get deps for the target
            let deps = get_deps(&target);
            let mut removable_deps = Vec::new();

            // Try removing each dep
            for dep in deps {
                if test_passes_without_dep(&target, &dep, &test) {
                    removable_deps.push(dep);
                }
            }

            // Print results
            if removable_deps.is_empty() {
                println!("\nNo removable dependencies found.");
            } else {
                println!("\nThe following dependencies can potentially be removed:");
                for dep in removable_deps {
                    println!("  {}", dep);
                }
            }
        }
        Commands::TriggerScores {
            workspace_path: workspace_root,
            target,
            max_history_length,
            deps_file,
        } => {
            let deps_graph = if let Some(deps_file) = deps_file {
                bazel::BazelDependencyGraph::from_file(&deps_file)
            } else {
                bazel::BazelDependencyGraph::from_workspace(&workspace_root)
            };

            println!(
                "Calculating trigger scores for target in dir {}: {}",
                target, workspace_root
            );
            let trigger_score =
                calculate_trigger_scores(&workspace_root, &target, max_history_length);
            println!("Trigger score for {}: {}", target, trigger_score);
        }
        Commands::AnalyzeGitRepo {
            workspace_path,
            output,
            max_history_length,
        } => {
            let repo = git::GitRepo::from_path(&workspace_path, max_history_length).unwrap();
            let json = serde_json::to_string_pretty(&repo).unwrap();

            let mut file = File::create(output).unwrap();
            file.write_all(json.as_bytes()).unwrap();
        }
    }
}

fn get_deps(target: &str) -> Vec<String> {
    let cmd_args = ["print deps", target];
    info!("Executing: buildozer {}", cmd_args.join(" "));

    let output = Command::new("buildozer")
        .args(cmd_args)
        .output()
        .expect("Failed to execute buildozer");

    if !output.status.success() {
        error!(
            "buildozer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect()
}

fn remove_dep(target: &str, dep: &str) -> bool {
    let cmd = format!("remove deps {}", dep);
    info!("Executing: buildozer {} {}", cmd, target);

    let output = Command::new("buildozer")
        .args([&cmd, target])
        .output()
        .expect("Failed to execute buildozer");

    if !output.status.success() {
        error!(
            "buildozer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }

    true
}

fn add_dep(target: &str, dep: &str) -> bool {
    let cmd = format!("add deps {}", dep);
    info!("Executing: buildozer {} {}", cmd, target);

    let output = Command::new("buildozer")
        .args([&cmd, target])
        .output()
        .expect("Failed to execute buildozer");

    if !output.status.success() {
        error!(
            "buildozer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }

    true
}

fn test_passes_without_dep(target: &str, dep: &str, test_targets: &Vec<String>) -> bool {
    remove_dep(target, dep);
    let mut success = true;
    for test in test_targets {
        info!("Executing: bazel test {}", test);

        let output = Command::new("bazel")
            .args(["test", test])
            .output()
            .expect("Failed to execute bazel");

        if !output.status.success() {
            success = false;
            error!(
                "bazel test failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    // re-add the dep at the end
    add_dep(target, dep);
    success
}

fn calculate_trigger_scores(workspace_root: &str, target: &str, max_history_length: i64) -> usize {
    let source_files = get_source_files(workspace_root, target);
    let repo = git::GitRepo::from_path(workspace_root, max_history_length).unwrap();
    let mut all_commits: std::collections::HashSet<String> = std::collections::HashSet::new();
    for source_file in source_files {
        // println!("Analyzing source file: {}", source_file);
        if let Some(file) = repo.files.get(&source_file) {
            // println!("Found {} commits for {}", commits.len(), source_file);
            all_commits.extend(file.commit_history.iter().cloned());
        }
    }
    return all_commits.len();
}

fn get_source_files(workspace_root: &str, target: &str) -> std::collections::HashSet<String> {
    let output = Command::new("bazel")
        .current_dir(workspace_root)
        .args([
            "cquery",
            &format!("kind(\"source file\", deps({}))", target),
        ])
        .output()
        .expect("Failed to execute bazel cquery");

    if !output.status.success() {
        error!(
            "bazel cquery failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return std::collections::HashSet::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if line.starts_with("@") {
                return None;
            }
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                let s = format!("{}/{}", parts[0], parts[1]);
                // Remove the first two and the last " (null)"
                Some(s[2..s.len() - 7].to_string())
            } else {
                None
            }
        })
        .collect()
}
