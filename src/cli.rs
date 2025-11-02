use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum CargoCli {
    PatchSource(Cli),
}

#[derive(Parser)]
#[command(name = "patch-source")]
#[command(version, about = "Automatically apply dependency patch sections to Cargo.toml", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Apply patches from a source to the current Cargo.toml
    Apply {
        /// Local path to a workspace
        #[arg(long, conflicts_with = "git")]
        path: Option<PathBuf>,

        /// Git repository URL
        #[arg(long, conflicts_with = "path")]
        git: Option<String>,

        /// Git branch to use (only with --git)
        #[arg(long, requires = "git")]
        branch: Option<String>,

        /// Git tag to use (only with --git)
        #[arg(long, requires = "git", conflicts_with = "branch")]
        tag: Option<String>,

        /// Git revision to use (only with --git)
        #[arg(long, requires = "git", conflicts_with_all = ["branch", "tag"])]
        rev: Option<String>,

        /// Pattern to filter crates (e.g., "rattler-*")
        #[arg(long)]
        pattern: Option<String>,

        /// Path to Cargo.toml to modify (defaults to current directory)
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },

    /// Remove patches from the current Cargo.toml
    Remove {
        /// Pattern to filter which patches to remove
        #[arg(long)]
        pattern: Option<String>,

        /// Path to Cargo.toml to modify (defaults to current directory)
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
}
