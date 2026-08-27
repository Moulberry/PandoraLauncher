use gix::bstr::ByteSlice;
use std::path;

#[derive(Debug)]
#[cfg_attr(all(feature = "gix", feature = "git2"), allow(dead_code))]
pub enum GixError {
    Discover(Box<gix::discover::Error>),
    Describe(Box<gix::commit::describe::Error>),
    HeadCommit(gix::reference::head_commit::Error),
    FindExisting(gix::reference::find::existing::Error),
    IsDirty(Box<gix::status::is_dirty::Error>),
    Peel(gix::head::peel::Error),
}

impl From<gix::discover::Error> for GixError {
    fn from(e: gix::discover::Error) -> Self {
        Self::Discover(Box::new(e))
    }
}

impl From<gix::status::is_dirty::Error> for GixError {
    fn from(e: gix::status::is_dirty::Error) -> Self {
        Self::IsDirty(Box::new(e))
    }
}

impl From<gix::commit::describe::Error> for GixError {
    fn from(e: gix::commit::describe::Error) -> Self {
        Self::Describe(Box::new(e))
    }
}

impl From<gix::reference::find::existing::Error> for GixError {
    fn from(e: gix::reference::find::existing::Error) -> Self {
        Self::FindExisting(e)
    }
}

impl From<gix::head::peel::Error> for GixError {
    fn from(e: gix::head::peel::Error) -> Self {
        Self::Peel(e)
    }
}

impl From<gix::reference::head_commit::Error> for GixError {
    fn from(e: gix::reference::head_commit::Error) -> Self {
        Self::HeadCommit(e)
    }
}

/// Retrieves the git-tag or hash describing the exact version and a boolean
/// that indicates if the repository currently has dirty/staged files.
///
/// If a valid git-repo can't be discovered at or above the given path,
/// or if any operation on the repository fails, `None` is returned.
pub fn get_repo_description(
    manifest_location: &path::Path,
) -> Result<Option<(String, bool)>, GixError> {
    match gix::discover(manifest_location) {
        Ok(repo) => {
            let commit = repo.head_commit()?;
            let tag = commit.describe().format()?.to_string();
            let dirty = repo.is_dirty()?;
            Ok(Some((tag, dirty)))
        }
        Err(gix::discover::Error::Discover(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Retrieves the branch name and hash of HEAD.
///
/// The returned value is a tuple of head's reference-name, long-hash and short-hash. The
/// branch name will be `None` if the head is detached, or it's not valid UTF-8.
///
/// If a valid git-repo can't be discovered at or above the given path,
/// or if any operation on the repository fails, `None` is returned.
pub fn get_repo_head(
    manifest_location: &path::Path,
) -> Result<Option<(Option<String>, String, String)>, GixError> {
    match gix::discover(manifest_location) {
        Ok(repo) => {
            let mut head = repo.head()?;
            let branch = head
                .clone()
                .referent_name()
                .and_then(|rn| rn.as_bstr().to_str().ok().map(ToOwned::to_owned));
            if let Some(commit_id) = head.try_peel_to_id()? {
                let commit_id_short = commit_id.shorten_or_id().to_string();
                Ok(Some((branch, commit_id.to_string(), commit_id_short)))
            } else {
                Ok(None)
            }
        }
        Err(gix::discover::Error::Discover(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
