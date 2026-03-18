use std::{path::Path, sync::Arc};
use strum::{Display, EnumIter};

#[derive(Debug)]
pub struct ImportFromOtherLauncherJob {
    pub import_accounts: bool,
    pub root: Arc<Path>,
    pub paths: Vec<Arc<Path>>,
}

#[derive(Debug, Display, Clone, Copy, enum_map::Enum, EnumIter)]
pub enum OtherLauncher {
    Prism,
    CurseForge,
    Modrinth,
    MultiMC,
    ATLauncher,
}

impl OtherLauncher {
    pub fn default_path(&self, directories: &directories::BaseDirs) -> Arc<Path> {
        let data_dir = directories.data_dir();
        let document_dir = directories.home_dir().join("Documents");
        match self {
            OtherLauncher::Prism => data_dir.join("PrismLauncher").into(),
            OtherLauncher::CurseForge => document_dir.join("curseforge").join("minecraft").into(),
            OtherLauncher::Modrinth => data_dir.join("ModrinthApp").into(),
            OtherLauncher::MultiMC => data_dir.join("multimc").into(),
            OtherLauncher::ATLauncher => data_dir.join("atlauncher").into(),
        }
    }
}
