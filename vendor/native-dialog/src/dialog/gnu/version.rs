use std::cmp::Ordering;

use versions::SemVer;

#[derive(Debug)]
pub struct Version(SemVer);

impl Version {
    pub fn new(parts: (u32, u32, u32)) -> Self {
        Self(SemVer {
            major: parts.0,
            minor: parts.1,
            patch: parts.2,
            pre_rel: None,
            meta: None,
        })
    }

    pub fn parse(s: &str) -> Option<Self> {
        let semver = SemVer::new(s.trim_ascii())?;
        Some(Self(semver))
    }
}

impl PartialEq<(u32, u32, u32)> for Version {
    fn eq(&self, parts: &(u32, u32, u32)) -> bool {
        let other = Version::new(*parts);
        self.0 == other.0
    }
}

impl PartialOrd<(u32, u32, u32)> for Version {
    fn partial_cmp(&self, parts: &(u32, u32, u32)) -> Option<Ordering> {
        let other = Version::new(*parts);
        self.0.partial_cmp(&other.0)
    }
}
