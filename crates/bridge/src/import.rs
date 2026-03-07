use std::path::PathBuf;
use strum::{Display, EnumIter};

#[derive(Default, Debug)]
pub struct ImportFromOtherLaunchers {
    pub imports: enum_map::EnumMap<OtherLauncher, Option<ImportFromOtherLauncher>>,
}

#[derive(Debug, Default)]
pub struct ImportFromOtherLauncher {
    pub can_import_accounts: bool,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct ImportFromCustomPath {
	pub paths: Vec<PathBuf>,
	pub launcher_type: OtherLauncher,
}

#[derive(Debug, Display, Clone, Copy, enum_map::Enum, EnumIter, PartialEq)]
pub enum OtherLauncher {
	AtLauncher,
    Prism,
    Modrinth,
    MultiMC,
    Custom,
}

pub struct OtherLauncherImportData {
	pub launcher_type: OtherLauncher,
	pub can_import_accounts: bool,
	pub paths: Vec<PathBuf>,
}
