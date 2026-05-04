use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::backend::{Backend, BackendKind};
use crate::Result;
use crate::dialog::{
    DialogImpl, FileFiltersBag, OpenMultipleFile, OpenSingleDir, OpenSingleFile, SaveSingleFile,
};
use crate::utils::resolve_tilde;

impl OpenSingleFile {
    fn create(&self) -> Result<Backend> {
        let target = get_target(&self.location, &self.filename);

        let params = BackendParams {
            target: target.as_deref(),
            filters: &self.filters,
            multiple: false,
            dir: false,
            save: false,
            title: &self.title,
            owner: unsafe { self.owner.as_x11() },
        };

        init_backend(params)
    }
}

impl DialogImpl for OpenSingleFile {
    fn show(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.exec()?;
        Ok(output.map(parse_output))
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.spawn().await?;
        Ok(output.map(parse_output))
    }
}

impl OpenMultipleFile {
    fn create(&self) -> Result<Backend> {
        let target = get_target(&self.location, &self.filename);

        let params = BackendParams {
            target: target.as_deref(),
            filters: &self.filters,
            multiple: true,
            dir: false,
            save: false,
            title: &self.title,
            owner: unsafe { self.owner.as_x11() },
        };

        init_backend(params)
    }
}

impl DialogImpl for OpenMultipleFile {
    fn show(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.exec()?;

        let paths = match output {
            Some(output) => output
                .split(|c| *c == b'\n')
                .filter(|c| !c.is_empty())
                .map(parse_output)
                .collect(),
            None => vec![],
        };

        Ok(paths)
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.spawn().await?;

        let paths = match output {
            Some(output) => output
                .split(|c| *c == b'\n')
                .filter(|c| !c.is_empty())
                .map(parse_output)
                .collect(),
            None => vec![],
        };

        Ok(paths)
    }
}

impl OpenSingleDir {
    fn create(&self) -> Result<Backend> {
        let target = get_target(&self.location, &self.filename);

        let params = BackendParams {
            target: target.as_deref(),
            filters: &FileFiltersBag::default(),
            multiple: false,
            dir: true,
            save: false,
            title: &self.title,
            owner: unsafe { self.owner.as_x11() },
        };

        init_backend(params)
    }
}

impl DialogImpl for OpenSingleDir {
    fn show(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.exec()?;
        Ok(output.map(parse_output))
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.spawn().await?;
        Ok(output.map(parse_output))
    }
}

impl SaveSingleFile {
    fn create(&self, target: &Option<PathBuf>) -> Result<Backend> {
        let params = BackendParams {
            target: target.as_deref(),
            filters: &self.filters,
            multiple: false,
            dir: false,
            save: true,
            title: &self.title,
            owner: unsafe { self.owner.as_x11() },
        };

        init_backend(params)
    }

    fn warn(&self, path: &Path) -> Result<Backend> {
        let message = match path.extension() {
            None => String::from("Unrecognized file type. Please try again."),
            Some(ext) => {
                let ext = ext.to_string_lossy();
                format!("Unrecognized file type: {ext}. Please try again.")
            }
        };

        let mut backend = Backend::new()?;
        match backend.kind {
            BackendKind::KDialog => {
                backend.command.args([
                    "--msgbox",
                    &message,
                    "--title",
                    "Warning",
                    "--icon=dialog-warning",
                ]);
            }
            BackendKind::Zenity => {
                backend.command.args([
                    "--width=400",
                    "--warning",
                    "--title",
                    "Warning",
                    "--text",
                    &message,
                ]);
            }
            BackendKind::Yad => {
                backend.command.args([
                    "--button",
                    "Ok:0",
                    "--image",
                    "dialog-warning",
                    "--title",
                    "Warning",
                    "--text",
                    &message,
                ]);
            }
        };

        Ok(backend)
    }
}

impl DialogImpl for SaveSingleFile {
    fn show(self) -> Result<Self::Output> {
        let mut target = get_target(&self.location, &self.filename);

        loop {
            let backend = self.create(&target)?;
            let output = backend.exec()?;

            let Some(path) = output.map(parse_output) else {
                break Ok(None);
            };

            if self.filters.accepts(&path) {
                break Ok(Some(path));
            }

            self.warn(&path)?.exec()?;

            target = Some(path);
        }
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        let mut target = get_target(&self.location, &self.filename);

        loop {
            let backend = self.create(&target)?;
            let output = backend.spawn().await?;

            let Some(path) = output.map(parse_output) else {
                break Ok(None);
            };

            if self.filters.accepts(&path) {
                break Ok(Some(path));
            }

            self.warn(&path)?.spawn().await?;

            target = Some(path);
        }
    }
}

fn parse_output(buf: impl AsRef<[u8]>) -> PathBuf {
    let bytes = buf.as_ref().trim_ascii();
    PathBuf::from(OsStr::from_bytes(bytes))
}

fn get_target(location: &Option<PathBuf>, filename: &Option<String>) -> Option<PathBuf> {
    let location = location.as_deref().and_then(resolve_tilde);
    let filename = filename.as_deref();

    match location {
        Some(location) => Some(location.join(filename.unwrap_or(""))),
        None => filename.map(PathBuf::from),
    }
}

struct BackendParams<'a> {
    target: Option<&'a Path>,
    filters: &'a FileFiltersBag,
    multiple: bool,
    dir: bool,
    save: bool,
    title: &'a str,
    owner: Option<u64>,
}

fn init_backend(params: BackendParams) -> Result<Backend> {
    let mut backend = Backend::new()?;
    match backend.kind {
        BackendKind::KDialog => init_kdialog(&mut backend, params),
        BackendKind::Zenity => init_zenity(&mut backend, params),
        BackendKind::Yad => init_yad(&mut backend, params),
    };

    Ok(backend)
}

fn init_kdialog(backend: &mut Backend, params: BackendParams) {
    if let Some(owner) = params.owner {
        backend.command.arg(format!("--attach=0x{:x}", owner));
    }

    match (params.dir, params.save) {
        (false, false) => backend.command.arg("--getopenfilename"),
        (false, true) => backend.command.arg("--getsavefilename"),
        (true, false) => backend.command.arg("--getexistingdirectory"),
        (true, true) => unreachable!(),
    };

    backend.command.arg("--title");
    backend.command.arg(params.title);

    match params.target {
        Some(path) => backend.command.arg(path),
        None => backend.command.arg(""),
    };

    if params.multiple {
        backend.command.args(["--multiple", "--separate-output"]);
    }

    if !params.filters.items.is_empty() {
        let filters: Vec<String> = params
            .filters
            .items
            .iter()
            .map(|filter| filter.format("{name} ({types})", "*{ext}", " "))
            .collect();

        backend.command.arg(filters.join("\n"));
    }
}

fn init_zenity(backend: &mut Backend, params: BackendParams) {
    let version = backend.version();

    if let Some(owner) = params.owner {
        // `--attach` was removed after zenity 3.44.3
        // https://github.com/GNOME/zenity/commit/cbf1311
        if matches!(&version, Some(v) if *v <= (3, 44, 2)) {
            backend.command.arg(format!("--attach=0x{:x}", owner));
        }
    }

    backend.command.arg("--file-selection");

    backend.command.arg("--title");
    backend.command.arg(params.title);

    if params.dir {
        backend.command.arg("--directory");
    }

    if params.save {
        backend.command.arg("--save");

        // `--confirm-overwrite` was removed at zenity 3.91.0
        // https://gitlab.gnome.org/GNOME/zenity/-/issues/55
        if matches!(&version, Some(v) if *v < (3, 91, 0)) {
            backend.command.arg("--confirm-overwrite");
        }
    };

    if params.multiple {
        backend.command.args(["--multiple", "--separator", "\n"]);
    }

    if let Some(path) = params.target {
        backend.command.arg("--filename");
        backend.command.arg(path);
    }

    if !params.filters.items.is_empty() {
        for filter in &params.filters.items {
            let formatted = filter.format("{name} ({types}) | {types}", "*{ext}", " ");
            backend.command.arg("--file-filter");
            backend.command.arg(formatted);
        }
    }
}

fn init_yad(backend: &mut Backend, params: BackendParams) {
    backend.command.arg("--file");

    backend.command.arg("--width");
    backend.command.arg("700");

    backend.command.arg("--title");
    backend.command.arg(params.title);

    if params.dir {
        backend.command.arg("--directory");
    }

    if params.save {
        backend.command.arg("--save");
        backend.command.arg("--confirm-overwrite");
    };

    if params.multiple {
        backend.command.args(["--multiple", "--separator", "\n"]);
    }

    if let Some(path) = params.target {
        backend.command.arg("--filename");
        backend.command.arg(path);
    }

    if !params.filters.items.is_empty() {
        for filter in &params.filters.items {
            let formatted = filter.format("{name} ({types}) | {types}", "*{ext}", " ");
            backend.command.arg("--file-filter");
            backend.command.arg(formatted);
        }
    }
}
