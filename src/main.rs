use clap::Parser;
use std::process::Command;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Analyzes and identifies removable Bazel dependencies"
)]
struct Args {
    /// The target to analyze
    target: String,

    /// Test targets to verify against
    #[arg(long, required = true)]
    test: Vec<String>,
}

fn main() {
    let args = Args::parse();

    println!("Analyzing target: {}", args.target);
    println!("Test targets:");
    for test in &args.test {
        println!("  {}", test);
    }

    // Get deps for the target
    let deps = get_deps(&args.target);
    let mut removable_deps = Vec::new();

    // Try removing each dep
    for dep in deps {
        if test_passes_without_dep(&args.target, &dep, &args.test) {
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

fn get_deps(target: &str) -> Vec<String> {
    let output = Command::new("buildozer")
        .args(["print deps", target])
        .output()
        .expect("Failed to execute buildozer");

    if !output.status.success() {
        eprintln!(
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
    let output = Command::new("buildozer")
        .args(["remove deps", dep, target])
        .output()
        .expect("Failed to execute buildozer");

    if !output.status.success() {
        eprintln!(
            "buildozer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }

    true
}

fn add_dep(target: &str, dep: &str) -> bool {
    let output = Command::new("buildozer")
        .args(["add deps", dep, target])
        .output()
        .expect("Failed to execute buildozer");

    if !output.status.success() {
        eprintln!(
            "buildozer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }

    true
}

fn test_passes_without_dep(target: &str, dep: &str, test_targets: &[&str]) -> bool {
    remove_dep(target, dep);
    let mut success = true;
    for test in test_targets {
        let output = Command::new("bazel")
            .args(["test", test])
            .output()
            .expect("Failed to execute bazel");

        if !output.status.success() {
            success = false;
            eprintln!(
                "bazel test failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    // re-add the dep at the end
    add_dep(target, dep);
    success
}
