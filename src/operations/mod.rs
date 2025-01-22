//! Operations module for interacting with Buildozer and Bazel commands.
//!
//! This module provides functions to manage dependencies in Bazel BUILD files using
//! the Buildozer tool. It includes functionality to:
//! * Query dependencies for a target
//! * Add and remove dependencies
//! * Test if a target's tests pass without a specific dependency
//!
//! All functions in this module expect the `buildozer` and `bazel` commands to be
//! available in the system path.
use std::process::Command;
use tracing::{error, info};

pub fn get_deps(target: &str) -> Vec<String> {
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

pub fn remove_dep(target: &str, dep: &str) -> bool {
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

pub fn add_dep(target: &str, dep: &str) -> bool {
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

pub fn test_passes_without_dep(target: &str, dep: &str, test_targets: &Vec<String>) -> bool {
    remove_dep(target, dep);
    let mut success = true;
    for test in test_targets {
        info!("executing: bazel test {}", test);

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
