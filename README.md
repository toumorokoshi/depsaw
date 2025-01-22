# depsaw

A tool to analyze dependencies for bazel projects.

## What is depsaw?

**note**: this project is experimental. The API is subject to change. Feel free
to contribute or file an issue!

Depsaw helps you find unused dependencies in your bazel project.

## Installation

See the [releases page](https://github.com/toumorokoshi/depsaw/releases) to grab
the precompiled binary.

To build from source, clone the repository, install
[Cargo](https://doc.rust-lang.org/cargo/), and run:

```bash
cargo build --release
./target/release/depsaw # built binary
```

## User Guide

In the future, depsaw will include commands that will be able to automatically
reduce building.

Currently, it provides various utilities to help analyze dependencies and
rebuild statistics, and provide insights to optimize them.

### Figure out what bazel targets are causing the most issues

Run the following:

```bash
TARGET=YOUR_TARGET_HERE
depsaw analyze --workspace-root ${WORKSPACE_ROOT} --target "${BAZEL_TARGET}" trigger-scores-map > /tmp/map.yaml
```

Or you can apply other strategies:

```bash
depsaw analyze --target="//:srcs" --workspace-root ~/workspace/bazel most-unique-triggers
```

Run `depsaw analyze --help` for a list of all commands.

### Pre-cache git and bazel analysis

Sometimes, git and bazel repositories can take a long time to analyze, such that
you may want to re-use those results.


1. Build the dependency graph for your large repository, and store that
   relationship:

```bash
TARGET="//..."
DEPS_FILE=/tmp/deps.rkyv
depsaw analyze-bazel-deps $(pwd) "${TARGET}" --output "${DEPS_FILE}"
```

1b. Optional, but if you'd like, you can pre-calculate the modified files as well:

```bash
depsaw run analyze-git-repo $(pwd) --output /tmp/git-analysis.rkyv
```

You can pass that in via the `--git-analysis-file` argument in analyze:

```bash
TARGET=YOUR_TARGET_HERE
depsaw analyze --bazel-analysis-file ${BAZEL_ANALYSIS_FILE} --git-analysis-file ${GIT_ANALYSIS_FILE} --target "${BAZEL_TARGET}" trigger-scores-map > /tmp/map.yaml
```