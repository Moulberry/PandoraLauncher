use std::path::PathBuf;
use strum::{Display, EnumIter};

#[derive(Default, Debug)]
pub struct ImportFromOtherLaunchers {
    pub imports: enum_map::EnumMap<OtherLauncher, Option<ImportFromOtherLauncher>>,
}

#[derive(Debug)]
pub struct ImportFromOtherLauncher {
    pub can_import_accounts: bool,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Display, Clone, Copy, enum_map::Enum, EnumIter)]
pub enum OtherLauncher {
    Prism,
    Modrinth,
    MultiMC,
    ATLauncher,
}
