use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};

const LAUNCH_WRAPPER: &[u8] = include_bytes!("../../../wrapper/LaunchWrapper.jar");

pub fn create_wrapper(temp_dir: &Path) -> PathBuf {
    let mut hasher = Sha1::new();
    hasher.update(LAUNCH_WRAPPER);
    let hash = hasher.finalize();

    let hash = hex::encode(hash);
    let launch_wrapper = temp_dir.join(format!("LaunchWrapper-{}.jar", hash));

    if !launch_wrapper.exists() {
        log::info!("Writing launch wrapper jar to {:?}", launch_wrapper);
        _ = crate::fs::write_safe(&launch_wrapper, LAUNCH_WRAPPER);
    }

    launch_wrapper
}
