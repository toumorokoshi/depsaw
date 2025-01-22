use clap::Parser;
use rkyv;
use std::error::Error;
use std::fs::File;
use std::io::Write;

mod algorithms;
mod bazel;
mod git;
mod operations;
use tracing::info;
use tracing_subscriber;
use tracing_subscriber::filter::LevelFilter;

// we declare a macro since the Serializable trait
// cannot be used as a function argument.
macro_rules! serialize {
    ( $object:expr, $format:expr) => {
        match $format.as_str() {
            "yaml" => {
                let yaml_output = serde_yaml::to_string(&$object)?;
                println!("{}", yaml_output);
            }
            "csv" => {
                let mut wtr = csv::WriterBuilder::new()
                    .has_headers(false)
                    .from_writer(std::io::stdout());
                // Serialize each target as a row
                for item in $object {
                    wtr.serialize(item)?;
                }
                wtr.flush()?;
            }
            _ => {
                panic!("Unsupported format: {}", $format);
            }
        }
    };
}

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Analyzes and identifies removable Bazel dependencies"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Precalculate data needed for analysis
    Precalculate {
        /// Path to output file
        #[arg(long, required = true)]
        output: String,

        #[command(subcommand)]
        command: PrecalculateCommands,
    },
    /// Run analysis algorithms
    Analyze {
        /// Path to the workspace root
        #[arg(long)]
        workspace_root: Option<String>,

        /// The target to analyze
        #[arg(long)]
        target: String,

        /// Specified, via git's `since` format, which commits to evaluate
        #[arg(long)]
        since: Option<String>,

        /// Path to the git analysis file
        #[arg(long)]
        git_analysis_file: Option<String>,

        /// Path to the bazel analysis filefile
        #[arg(long)]
        bazel_analysis_file: Option<String>,

        /// The format to output the results in
        #[arg(long, default_value = "yaml")]
        format: String,

        #[command(subcommand)]
        algorithm: AnalyzeCommands,
    },
}

#[derive(clap::Subcommand)]
enum PrecalculateCommands {
    /// Analyze git repository data
    GitRepo {
        /// Path to the workspace root
        #[arg(long, required = true)]
        workspace_root: String,

        /// The maximum number of commit history to consider
        #[arg(long)]
        since: Option<String>,
    },
    /// Analyze Bazel dependency graph
    BazelDeps {
        /// Path to the workspace root
        #[arg(long, required = true)]
        workspace_root: String,

        /// The target to analyze
        #[arg(long, required = true)]
        target: String,
    },
}

#[derive(clap::Subcommand)]
enum AnalyzeCommands {
    /// Generate trigger scores map
    TriggerScoresMap {},
    /// Find most unique triggers
    MostUniqueTriggers {},
    /// Analyze removable dependencies
    RemovableDeps {
        /// Test targets to verify against
        #[arg(long, required = true)]
        test: Vec<String>,
    },
}

fn main() {
    main_inner().unwrap();
}

fn main_inner() -> anyhow::Result<(), Box<dyn Error>> {
    setup()?;
    let args = Args::parse();
    info!("Starting analysis");

    match args.command {
        Commands::Precalculate { output, command } => match command {
            PrecalculateCommands::GitRepo {
                workspace_root,
                since,
            } => {
                let repo = git::GitRepo::from_path(&workspace_root, since).unwrap();
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&repo)?;
                let mut file = File::create(output).unwrap();
                file.write_all(&bytes).unwrap();
                Ok(())
            }
            PrecalculateCommands::BazelDeps {
                workspace_root,
                target,
            } => {
                let deps_graph =
                    bazel::BazelDependencyGraph::from_workspace(&workspace_root, &target)?;
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&deps_graph)?;
                let mut file = File::create(output).unwrap();
                file.write_all(&bytes).unwrap();
                Ok(())
            }
        },
        Commands::Analyze {
            workspace_root,
            target,
            since,
            git_analysis_file,
            bazel_analysis_file,
            format,
            algorithm,
        } => {
            let workspace_root = workspace_root.unwrap_or_else(|| ".".to_string());

            // Load dependencies
            let deps_graph = if let Some(deps_file) = bazel_analysis_file {
                bazel::BazelDependencyGraph::from_file(&deps_file)?
            } else {
                bazel::BazelDependencyGraph::from_workspace(&workspace_root, &target)?
            };

            // Load git repo
            let repo = if let Some(git_analysis_file) = git_analysis_file {
                git::GitRepo::from_file(&git_analysis_file).unwrap()
            } else {
                git::GitRepo::from_path(&workspace_root, since).unwrap()
            };

            match algorithm {
                AnalyzeCommands::TriggerScoresMap {} => {
                    let scores_by_target =
                        algorithms::calculate_trigger_scores(&target, &repo, &deps_graph)?;
                    let mut sorted_scores: Vec<_> = scores_by_target.iter().collect();
                    sorted_scores.sort_by(|a, b| b.1.cmp(a.1));
                    serialize!(sorted_scores, format);
                    Ok(())
                }
                AnalyzeCommands::MostUniqueTriggers {} => {
                    let results = algorithms::most_unique_triggers(&target, &repo, &deps_graph)?;
                    serialize!(results, format);
                    Ok(())
                }
                // TODO: move this to it's own operations subcommands
                AnalyzeCommands::RemovableDeps { test } => {
                    info!("Analyzing target: {}", target);
                    info!("Test targets:");
                    for test_target in &test {
                        info!("  {}", test_target);
                    }

                    // Get deps for the target
                    let deps = operations::get_deps(&target);
                    let mut removable_deps = Vec::new();

                    // Try removing each dep
                    for dep in deps {
                        if operations::test_passes_without_dep(&target, &dep, &test) {
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
                    Ok(())
                }
            }
        }
    }
}

fn setup() -> Result<(), Box<dyn Error>> {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
