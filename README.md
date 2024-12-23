# depsaw

A tool to analyze dependencies for bazel projects.

## What is depsaw?

**note**: this project is experimental. The API is subject to change. Feel free
to contribute or file an issue!

Depsaw helps you find unused dependencies in your bazel project.

## Installation

See the releases page on GitHub to grab the precompiled binary.

Or clone the repository, install [Cargo](https://doc.rust-lang.org/cargo/), and run:

```bash
cargo build --release
./target/release/dephammer # built binary
```

## User Guide

In the future, depsaw will be able to help automatically analyze and reduce
dependencies.

For now, it provides various utilities to help analyze dependency and rebuild
statistics, and provide insights to optimize them.

### Figure out what bazel targets are causing the most issues

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

You can pass that in via the `--git-analysis-file` argument in
`trigger-scores-map`.

2. Run trigger-scores-map on specific target you care about - this will analyze
   the graph just for your dependencies.

```bash
TARGET=YOUR_TARGET_HERE
depsaw trigger-scores-map $(pwd) "${TARGET}" --format=csv --since 2024-11-01 --deps-file "${DEPS_FILE}" > /tmp/deps.csv
```

That will generate a csv, sorted by score, to find the targets that are
triggering the most builds.