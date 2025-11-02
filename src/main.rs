use cargo_patch_source::cli::{CargoCli, Commands};
use cargo_patch_source::source::{GitReference, PatchSource};
use cargo_patch_source::{apply_patches, remove_patches};
use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    miette::set_panic_hook();

    let CargoCli::PatchSource(cli) = CargoCli::parse();

    match cli.command {
        Commands::Apply {
            path,
            git,
            branch,
            tag,
            rev,
            pattern,
            manifest_path,
        } => {
            // Determine the source
            let source = if let Some(path) = path {
                PatchSource::local_path(path)
            } else if let Some(url) = git {
                let reference = if let Some(branch) = branch {
                    Some(GitReference::Branch(branch))
                } else if let Some(tag) = tag {
                    Some(GitReference::Tag(tag))
                } else {
                    rev.map(GitReference::Rev)
                };
                PatchSource::git(url, reference)
            } else {
                return Err(cargo_patch_source::PatchError::NoSourceSpecified.into());
            };

            apply_patches(source, manifest_path, pattern.as_deref())?;
        }
        Commands::Remove {
            pattern,
            manifest_path,
        } => {
            remove_patches(manifest_path, pattern.as_deref())?;
        }
    }

    Ok(())
}
