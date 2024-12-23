use log::{debug, info};
use rkyv;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::process::Command;

#[derive(Archive, Debug, RkyvSerialize, RkyvDeserialize, Clone)]
pub struct BazelDependencyGraph {
    pub rules_by_label: HashMap<String, Entry>,
}

#[derive(Archive, Debug, RkyvSerialize, RkyvDeserialize, Clone)]
pub struct Entry {
    pub dep_targets: Vec<String>,
    pub source_files: Vec<String>,
}

impl BazelDependencyGraph {
    pub fn from_file(path: &str) -> Result<BazelDependencyGraph, Box<dyn Error>> {
        info!("reading bazel dependency graph from {}", path);
        let content = std::fs::read(path).unwrap();
        Ok(rkyv::from_bytes::<BazelDependencyGraph, rkyv::rancor::Error>(&content)?)
    }

    pub fn from_workspace(workspace_root: &str, target: &str) -> BazelDependencyGraph {
        debug!("running bazel query");
        let output = Command::new("bazel")
            .current_dir(workspace_root)
            .args([
                "query",
                &format!("deps({})", target),
                "--output",
                "streamed_jsonproto",
            ])
            .output()
            .expect("Failed to execute bazel query");
        let content = String::from_utf8(output.stdout).unwrap();
        BazelDependencyGraph::from_string(&content)
    }

    pub fn from_string(content: &str) -> BazelDependencyGraph {
        info!("parsing bazel dependency graph");
        let raw_entries = read_from_protojson(content);
        let mut targets_by_label = HashMap::new();
        let mut rules = vec![];
        for entry in raw_entries {
            let name = match &entry {
                DependencyEntry::RULE { rule } => {
                    rules.push(rule.clone());
                    rule.name.clone()
                }
                DependencyEntry::SOURCE_FILE { sourceFile } => sourceFile.name.clone(),
                DependencyEntry::PACKAGE_GROUP { packageGroup } => packageGroup.name.clone(),
                DependencyEntry::GENERATED_FILE { generatedFile } => generatedFile.name.clone(),
            };
            targets_by_label.insert(name, entry);
        }
        let mut rules_by_label = HashMap::new();
        // parse through each rule
        for rule in rules {
            let mut source_files = vec![];
            let mut dep_targets = vec![];
            for dep in rule.ruleInput {
                // ignore external dependencies
                if dep.starts_with("@") {
                    continue;
                }
                if let Some(entry) = targets_by_label.get(&dep) {
                    match entry {
                        DependencyEntry::SOURCE_FILE { sourceFile } => {
                            source_files.push(sourceFile.name.clone());
                        }
                        DependencyEntry::RULE { rule } => {
                            dep_targets.push(rule.name.clone());
                        }
                        _ => {}
                    }
                }
            }
            let entry = Entry {
                dep_targets,
                source_files,
            };
            debug!("adding rule: {}", rule.name);
            rules_by_label.insert(rule.name, entry);
        }

        BazelDependencyGraph { rules_by_label }
    }

    pub fn get_source_files(
        &self,
        target: &str,
        recursive: bool,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let mut visited_targets = HashSet::new();
        self.get_source_files_inner(target, recursive, &mut visited_targets)
    }

    fn get_source_files_inner(
        &self,
        target: &str,
        recursive: bool,
        visited_targets: &mut HashSet<String>,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        debug!("getting source files for {}", target);
        let entry = self.rules_by_label.get(target).ok_or(format!(
            "target {} not found in bazel dependency graph",
            target
        ))?;
        let mut source_files = entry.source_files.clone();
        for dep_target in entry.dep_targets.iter() {
            if visited_targets.contains(dep_target) {
                continue;
            }
            source_files.extend(self.get_source_files_inner(
                dep_target,
                recursive,
                visited_targets,
            )?);
        }
        visited_targets.insert(target.to_string());
        Ok(source_files)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum DependencyEntry {
    RULE { rule: Rule },
    SOURCE_FILE { sourceFile: SourceFile },
    PACKAGE_GROUP { packageGroup: PackageGroup },
    GENERATED_FILE { generatedFile: GeneratedFile },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Rule {
    pub name: String,
    pub ruleClass: String,
    pub location: String,
    pub attribute: Vec<Attribute>,
    #[serde(default)]
    pub ruleInput: Vec<String>,
    #[serde(default)]
    pub ruleOutput: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Attribute {
    pub name: String,
    #[serde(rename = "type")]
    pub attr_type: String,
    #[serde(default)]
    pub stringValue: Option<String>,
    #[serde(default)]
    pub stringListValue: Option<Vec<String>>,
    #[serde(default)]
    pub intValue: Option<i64>,
    #[serde(default)]
    pub booleanValue: Option<bool>,
    pub explicitlySpecified: Option<bool>,
    pub nodep: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceFile {
    pub name: String,
    pub location: String,
    pub visibilityLabel: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PackageGroup {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeneratedFile {
    pub name: String,
    pub generatingRule: String,
    pub location: String,
}

// read the contents of a bazel protojson file and parse it into a vector of DependencyEntry
// this is generated via `bazel query "deps(//...)" --output streamed_jsonproto`
fn read_from_protojson(content: &str) -> Vec<DependencyEntry> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str(line)
                .map_err(|e| {
                    eprintln!("Failed to parse line: {}", e);
                    e
                })
                .ok()
        })
        .collect()
}
