use crate::{environment, fmt_option_str, write_variable};
use std::{fs, io, path};

/// Length of the short commit hash (8 characters by default)
const SHORT_HASH_LENGTH: usize = 8;

/// Fully resolved repository information, that may incorporate overrides.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct RepoInfo {
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub dirty: Option<bool>,
    pub commit_id: Option<String>,
    pub commit_id_short: Option<String>,
}

impl RepoInfo {
    pub(crate) fn from_overrides(envmap: &environment::EnvironmentMap) -> Self {
        RepoInfo {
            branch: envmap.get_override_var("GIT_HEAD_REF"),
            tag: envmap.get_override_var("GIT_VERSION"),
            dirty: envmap.get_override_var("GIT_DIRTY"),
            commit_id: envmap.get_override_var("GIT_COMMIT_HASH"),
            commit_id_short: envmap.get_override_var("GIT_COMMIT_HASH_SHORT"),
        }
    }
}

pub(crate) fn write_git_version(
    manifest_location: &path::Path,
    envmap: &environment::EnvironmentMap,
    w: &fs::File,
) -> io::Result<()> {
    #[cfg(feature = "git2")]
    use crate::git_impl::{get_repo_description, get_repo_head};
    // `git2` takes precedence
    #[cfg(all(feature = "gix", not(feature = "git2")))]
    use crate::gix_impl::{get_repo_description, get_repo_head};

    let RepoInfo {
        mut branch,
        mut tag,
        mut dirty,
        mut commit_id,
        mut commit_id_short,
    } = RepoInfo::from_overrides(envmap);

    if branch.is_none() || commit_id.is_none() || commit_id_short.is_none() {
        if let Ok(Some((git_branch, git_commit_id, git_commit_short_id))) =
            get_repo_head(manifest_location)
        {
            branch = branch.or(git_branch);
            commit_id = commit_id.or(Some(git_commit_id));
            commit_id_short = commit_id_short.or(Some(git_commit_short_id))
        }
    }

    // This is an expensive call, avoid it if it's all overridden.
    // TODO(performance): could be split into dirty + describe, and re-use the opened Repository.
    if tag.is_none() || dirty.is_none() {
        if let Ok(Some((git_tag, git_dirty))) = get_repo_description(manifest_location) {
            tag = tag.or(Some(git_tag));
            dirty = dirty.or(Some(git_dirty));
        }
    }

    write_variables(
        w,
        RepoInfo {
            branch,
            tag,
            dirty,
            commit_id,
            commit_id_short,
        },
    )
}

pub(crate) fn write_variables(
    mut w: &fs::File,
    RepoInfo {
        branch,
        tag,
        dirty,
        commit_id,
        mut commit_id_short,
    }: RepoInfo,
) -> io::Result<()> {
    use io::Write;

    // If we have a commit hash but no short hash, generate the short hash from the full hash
    if let (Some(h), None) = (&commit_id, &commit_id_short) {
        commit_id_short = Some(h.chars().take(SHORT_HASH_LENGTH).collect())
    }

    write_variable!(
        w,
        "GIT_VERSION",
        "Option<&str>",
        fmt_option_str(tag),
        "If the crate was compiled from within a git-repository, \
        `GIT_VERSION` contains HEAD's tag. The short commit id is used if HEAD is not tagged."
    );
    write_variable!(
        w,
        "GIT_DIRTY",
        "Option<bool>",
        match dirty {
            Some(true) => "Some(true)",
            Some(false) => "Some(false)",
            None => "None",
        },
        "If the repository had dirty/staged files."
    );

    let doc = "If the crate was compiled from within a git-repository, `GIT_HEAD_REF` \
        contains full name to the reference pointed to by HEAD \
        (e.g.: `refs/heads/master`). If HEAD is detached or the branch name is not \
        valid UTF-8 `None` will be stored.\n";
    write_variable!(
        w,
        "GIT_HEAD_REF",
        "Option<&str>",
        fmt_option_str(branch),
        doc
    );

    write_variable!(
        w,
        "GIT_COMMIT_HASH",
        "Option<&str>",
        fmt_option_str(commit_id),
        "If the crate was compiled from within a git-repository, `GIT_COMMIT_HASH` \
    contains HEAD's full commit SHA-1 hash."
    );

    write_variable!(
        w,
        "GIT_COMMIT_HASH_SHORT",
        "Option<&str>",
        fmt_option_str(commit_id_short),
        "If the crate was compiled from within a git-repository, `GIT_COMMIT_HASH_SHORT` \
    contains HEAD's short commit SHA-1 hash."
    );

    Ok(())
}

// Tests need git2 for initial setup right now.
#[cfg(feature = "git2")]
#[cfg(test)]
mod tests {
    #[cfg(not(feature = "gix"))]
    use crate::git_impl::{get_repo_description, get_repo_head};
    // When testing, `gix` must take precedence, or it's not tested.
    // Using `gix-testtools` would allow to setup fixtures without such dependencies.
    #[cfg(feature = "gix")]
    use crate::gix_impl::{get_repo_description, get_repo_head};

    #[test]
    fn parse_git_repo() {
        use std::fs;
        use std::path;

        let repo_root = tempfile::tempdir().unwrap();
        assert_eq!(get_repo_description(repo_root.as_ref()).unwrap(), None);

        let repo = git2::Repository::init_opts(
            &repo_root,
            git2::RepositoryInitOptions::new()
                .external_template(false)
                .mkdir(false)
                .no_reinit(true)
                .mkpath(false),
        )
        .unwrap();

        let cruft_file = repo_root.path().join("cruftfile");
        std::fs::write(&cruft_file, "Who? Me?").unwrap();

        let project_root = repo_root.path().join("project_root");
        fs::create_dir(&project_root).unwrap();

        let sig = git2::Signature::now("foo", "bar").unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(path::Path::new("cruftfile")).unwrap();
        idx.write().unwrap();
        let commit_oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Testing testing 1 2 3",
                &repo.find_tree(idx.write_tree().unwrap()).unwrap(),
                &[],
            )
            .unwrap();

        let binding = repo
            .find_commit(commit_oid)
            .unwrap()
            .into_object()
            .short_id()
            .unwrap();

        let commit_oid_short = binding.as_str().unwrap();

        let commit_hash = commit_oid.to_string();
        let commit_hash_short = commit_oid_short.to_string();

        assert!(commit_hash.starts_with(&commit_hash_short));

        // The commit, the commit-id is something and the repo is not dirty
        let (tag, dirty) = get_repo_description(&project_root).unwrap().unwrap();
        assert!(!tag.is_empty());
        assert!(!dirty);

        // Tag the commit, it should be retrieved
        repo.tag(
            "foobar",
            &repo
                .find_object(commit_oid, Some(git2::ObjectType::Commit))
                .unwrap(),
            &sig,
            "Tagged foobar",
            false,
        )
        .unwrap();

        let (tag, dirty) = get_repo_description(&project_root).unwrap().unwrap();
        assert_eq!(tag, "foobar");
        assert!(!dirty);

        // Make some dirt
        std::fs::write(cruft_file, "now dirty").unwrap();
        let (tag, dirty) = get_repo_description(&project_root).unwrap().unwrap();
        assert_eq!(tag, "foobar");
        assert!(dirty);

        let branch_short_name = "baz";
        let branch_name = "refs/heads/baz";
        let commit = repo.find_commit(commit_oid).unwrap();
        repo.branch(branch_short_name, &commit, true).unwrap();
        repo.set_head(branch_name).unwrap();

        assert_eq!(
            get_repo_head(&project_root).unwrap(),
            Some((Some(branch_name.to_owned()), commit_hash, commit_hash_short))
        );
    }

    #[test]
    fn detached_head_repo() {
        let repo_root = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init_opts(
            &repo_root,
            git2::RepositoryInitOptions::new()
                .external_template(false)
                .mkdir(false)
                .no_reinit(true)
                .mkpath(false),
        )
        .unwrap();
        let sig = git2::Signature::now("foo", "bar").unwrap();
        let commit_oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Testing",
                &repo
                    .find_tree(repo.index().unwrap().write_tree().unwrap())
                    .unwrap(),
                &[],
            )
            .unwrap();

        let binding = repo
            .find_commit(commit_oid)
            .unwrap()
            .into_object()
            .short_id()
            .unwrap();

        let commit_oid_short = binding.as_str().unwrap();

        let commit_hash = commit_oid.to_string();
        let commit_hash_short = commit_oid_short.to_string();

        assert!(commit_hash.starts_with(&commit_hash_short));

        repo.set_head_detached(commit_oid).unwrap();
        assert_eq!(
            get_repo_head(repo_root.as_ref()).unwrap(),
            Some((None, commit_hash, commit_hash_short))
        );
    }
}
