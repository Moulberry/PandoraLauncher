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
	pub instances: HashMap<PathBuf, ImportStatus>,
	pub account: Option<PathBuf>,
	// This is placeholder for a future update if we ever implement it.
	// might remove it before releasing this PR though... (if i don't, then i forgot.)
	// pub can_deduplicate: bool,
}

#[derive(Debug, Display, Clone, Copy, enum_map::Enum, EnumIter, PartialEq, Eq)]
pub enum OtherLauncher {
	AtLauncher,
    Prism,
    Modrinth,
    MultiMC,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportStatus {
	#[default]
	NotImporting,
	Importing,
	/// This means we already have an instance with this name, hence it's not worth importing again.
	Duplicate,
}

impl ImportStatus {
	pub fn flip(&mut self) {
	    *self = match *self {
	        ImportStatus::NotImporting => ImportStatus::Importing,
	        ImportStatus::Importing => ImportStatus::NotImporting,
	        ImportStatus::Duplicate => ImportStatus::Duplicate,
	    };
	}

	pub fn enable(&mut self) {
		*self = match *self {
	    	ImportStatus::NotImporting => ImportStatus::Importing,
	     	ImportStatus::Importing => ImportStatus::Importing,
	     	ImportStatus::Duplicate => ImportStatus::Duplicate,
	  	};
	}

	pub fn disable(&mut self) {
		*self = match *self {
		    ImportStatus::NotImporting => ImportStatus::NotImporting,
		    ImportStatus::Importing => ImportStatus::NotImporting,
		    ImportStatus::Duplicate => ImportStatus::Duplicate,
		};
	}
}
