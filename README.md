# dephammer

A jackhammer to remove deps for bazel projects

## What is dephammer?

Dephammer helps you find unused dependencies in your bazel project.

## User Guide

dephammer expects to run in the root of your bazel workspace.

Dephammer works on a specific bazel target, paired with a list of test targets. the invocation looks like:

```bash
dephammer //foo:build --test=//foo:build_test --test=//foo:build_test_2
```

From there, dephammer will:

1. look at all the deps and data entries of //foo:build.
2. remove them one by one.
3. run each of the test targets to see if they fail.
4. print a list of the deps in which the tests continued to pass, even after
   they were removed.


## Extracting