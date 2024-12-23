# depsaw

A jackhammer to remove deps for bazel projects

## What is depsaw?

Depsaw helps you find unused dependencies in your bazel project.

## Installation

See the releases page on GitHub to grab the precompiled binary.

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

2. Run trigger-scores-map on specific target you care about - this will analyze
   the graph just for your dependencies.

```bash
TARGET=YOUR_TARGET_HERE
depsaw trigger-scores-map $(pwd) "${TARGET}" --format=csv --since 2024-11-01 --deps-file "${DEPS_FILE}" > /tmp/deps.csv
```

Thaat will generate a csv, sorted by score, to find the targets that are
triggering the most builds.