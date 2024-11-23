#

## Brainstorming

dephammer will attempt to figure out which dependencies are the most valuable to
remove. It does not consider things like the actual cost to build a specific
target, and instead only works based on the *number* of targets.

Heuristically:
- the targets with the most `rdeps` are the ones that can trigger the most
  downstream changes.
- the targets with the most `deps` are the ones that are *triggered* the most.
- the impact of removing a particular dep is multiplied the by the frequency that the file has been modified.

A rough model would be:

```
cost(target) = change_frequency * rdeps
```

So the algorithm should look like:

1. calculate the cost of each target:
2. iterate for each target, in order of descending cost.
3. trim deps
4. back to 1

The "trim deps" phase can be partially automated - we could just try to remove deps, build all relevant test targets, and find the ones that pass even if removed. But most likely, manual effort will be required to actually remove targets.

One *could* use an additional heuristic that the number of test targets that fail is a reflection of how hard it is to remove a dep. So it would be helpful to log, after each attempted dep removal, how many test targets failed.

You would want to run all builds and tests affected by a target to verify the change.

- possibly the worst targets are those that have a larger number of deps *and* a large number or rdeps?

## Authoring the cost analyzer

The cost analyzer is probably the most valuable part. For a given target you want to optimize, you could.

1. traverse the tree to find each dep
2. list the source files for each "root" dep
3. see how often each file was modified over a time range (say, one year, or the whole history of the file)
3. each time the file is modified, it would count as 1 change.
4. sum the values for each downstream dep, so that one can see the total cost of each immediate dep of the specific target you want to optimize.

That will let you stack rank which one to try to trim. You could even sort all
deps by their cost, so that you can see if it's better to remove an immediate
dep, or factor out some intermediary dep.

### Figuring out files changed

With git, you can do:

```bash
git log --numstat | awk '/^[0-9-]+/{ print $NF}' | sort | uniq -c | sort -nr
```

Write this to a file to find the files most changed.

### finding all bazel targets

```
bazel query ...
```

## Other possible algorithms

- look at `somepath` between a target and one you know *shouldn't* build the target, and try to jackhammmer those dependencies out.


### Extracting the dependency graph

For large repositories, writing the dependency graph to a file and reusing it
may be significantly faster than running the command every time:

```
bazel query "deps(//...)" --output streamed_jsonproto >  ~/sandbox/dephammer.ndjson
```