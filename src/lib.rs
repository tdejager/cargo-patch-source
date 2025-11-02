pub mod cargo_ops;
pub mod cli;
pub mod error;
pub mod patch;
pub mod source;
pub mod toml_ops;

pub use error::{PatchError, Result};
pub use patch::{apply_patches, remove_patches};
pub use source::{GitReference, PatchSource};
