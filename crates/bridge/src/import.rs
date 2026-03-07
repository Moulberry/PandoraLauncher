use std::{collections::HashMap, path::PathBuf};
use strum::{Display, EnumIter};

#[derive(Default, Debug)]
pub struct ImportFromOtherLaunchers {
    pub imports: enum_map::EnumMap<OtherLauncher, Option<ImportFromOtherLauncher>>,
}

#[derive(Debug)]
pub struct ImportFromOtherLauncher {
    // launcher is duplicated here for when we send a custom path request.
	pub launcher: OtherLauncher,
	/// A list of instances we can import.
	/// State:
	/// - 0: Not Importing
	/// - 1: Importing
	/// - 2: Can't import due to duplicate instance.
	/// TODO: Make this into an enum stupid.
	pub instances: HashMap<PathBuf, u8>,
	pub account: Option<PathBuf>,
	// This is placeholder for a future update if we ever implement it.
	// might remove it before releasing this PR though... (if i don't, then i forgot.)
	// pub can_deduplicate: bool,
}

#[derive(Debug, Display, Clone, Copy, enum_map::Enum, EnumIter, PartialEq)]
pub enum OtherLauncher {
	AtLauncher,
    Prism,
    Modrinth,
    MultiMC,
}
