use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

pub struct BazelDependencyGraph {
    targets_by_label: HashMap<String, DependencyEntry>,
}

impl BazelDependencyGraph {
    pub fn from_file(path: &str) -> BazelDependencyGraph {
        let content = std::fs::read_to_string(path).unwrap();
        BazelDependencyGraph::from_string(&content)
    }

    pub fn from_workspace(workspace_root: &str, target: &str) -> BazelDependencyGraph {
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
        let raw_entries = read_from_protojson(content);
        let mut targets_by_label = HashMap::new();
        for entry in raw_entries {
            let name = match &entry {
                DependencyEntry::RULE { rule } => rule.name.clone(),
                DependencyEntry::SOURCE_FILE { sourceFile } => sourceFile.name.clone(),
                DependencyEntry::PACKAGE_GROUP { packageGroup } => packageGroup.name.clone(),
                DependencyEntry::GENERATED_FILE { generatedFile } => generatedFile.name.clone(),
            };
            debug!("Adding target {}", name);
            targets_by_label.insert(name, entry);
        }
        BazelDependencyGraph { targets_by_label }
    }

    pub fn get_source_files(&self, target: &str, recursive: bool) -> Vec<&SourceFile> {
        let mut source_files = vec![];
        debug!("Getting source files for {}", target);
        let entry = self.targets_by_label.get(target).unwrap();
        match entry {
            DependencyEntry::SOURCE_FILE { sourceFile } => source_files.push(sourceFile),
            DependencyEntry::RULE { rule } => {
                if recursive {
                    for input in rule.ruleInput.iter() {
                        source_files.extend(self.get_source_files(&input, true));
                    }
                }
            }
            DependencyEntry::PACKAGE_GROUP { packageGroup } => {}
            DependencyEntry::GENERATED_FILE { generatedFile } => {}
        };
        source_files
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
