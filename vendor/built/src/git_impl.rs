/// Retrieves the git-tag or hash describing the exact version and a boolean
/// that indicates if the repository currently has dirty/staged files.
///
/// If a valid git-repo can't be discovered at or above the given path,
/// `Ok(None)` is returned instead of an `Err`-value.
///
/// # Errors
/// Errors from `git2` are returned if the repository does exists at all.
#[cfg(feature = "git2")]
pub fn get_repo_description(root: &std::path::Path) -> Result<Option<(String, bool)>, git2::Error> {
    match git2::Repository::discover(root) {
        Ok(repo) => {
            let mut desc_opt = git2::DescribeOptions::new();
            desc_opt.describe_tags().show_commit_oid_as_fallback(true);
            let tag = repo
                .describe(&desc_opt)
                .and_then(|desc| desc.format(None))?;
            let mut st_opt = git2::StatusOptions::new();
            st_opt.include_ignored(false);
            st_opt.include_untracked(false);
            let dirty = repo
                .statuses(Some(&mut st_opt))?
                .iter()
                .any(|status| !matches!(status.status(), git2::Status::CURRENT));
            Ok(Some((tag, dirty)))
        }
        Err(ref e)
            if e.class() == git2::ErrorClass::Repository
                && e.code() == git2::ErrorCode::NotFound =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Retrieves the branch name and hash of HEAD.
///
/// The returned value is a tuple of head's reference-name, long-hash and short-hash. The
/// branch name will be `None` if the head is detached, or it's not valid UTF-8.
///
/// If a valid git-repo can't be discovered at or above the given path,
/// `Ok(None)` is returned instead of an `Err`-value.
///
/// # Errors
/// Errors from `git2` are returned if the repository does exists at all.
#[cfg(feature = "git2")]
pub fn get_repo_head(
    root: &std::path::Path,
) -> Result<Option<(Option<String>, String, String)>, git2::Error> {
    match git2::Repository::discover(root) {
        Ok(repo) => {
            // Supposed to be the reference pointed to by HEAD, but it's HEAD
            // itself, if detached
            let head_ref = repo.head()?;
            let branch = {
                // Check whether `head` is really the pointed to reference and
                // not HEAD itself.
                if repo.head_detached()? {
                    None
                } else {
                    head_ref.name().ok()
                }
            };
            let head = head_ref.peel_to_commit()?;
            let commit = head.id();
            let commit_short = head.into_object().short_id()?;
            Ok(Some((
                branch.map(ToString::to_string),
                format!("{commit}"),
                commit_short.as_str().unwrap_or_default().to_string(),
            )))
        }
        Err(ref e)
            if e.class() == git2::ErrorClass::Repository
                && e.code() == git2::ErrorCode::NotFound =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
