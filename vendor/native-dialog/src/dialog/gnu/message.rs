use std::collections::HashMap;

use super::backend::{Backend, BackendKind};
use crate::dialog::{DialogImpl, MessageAlert, MessageConfirm};
use crate::{MessageLevel, Result};

impl MessageAlert {
    fn create(&self) -> Result<Backend> {
        let params = BackendParams {
            title: &self.title,
            text: &self.text,
            level: self.level,
            ask: false,
            owner: unsafe { self.owner.as_x11() },
        };

        init_backend(params)
    }
}

impl DialogImpl for MessageAlert {
    fn show(self) -> Result<Self::Output> {
        let backend = self.create()?;
        backend.exec()?;
        Ok(())
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        let backend = self.create()?;
        backend.spawn().await?;
        Ok(())
    }
}

impl MessageConfirm {
    fn create(&self) -> Result<Backend> {
        let params = BackendParams {
            title: &self.title,
            text: &self.text,
            level: self.level,
            ask: true,
            owner: unsafe { self.owner.as_x11() },
        };

        init_backend(params)
    }
}

impl DialogImpl for MessageConfirm {
    fn show(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.exec()?;
        Ok(output.is_some())
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        let backend = self.create()?;
        let output = backend.spawn().await?;
        Ok(output.is_some())
    }
}

/// Modified version of `str::replace`.
fn replace_many(text: &str, replacements: HashMap<char, &str>) -> String {
    let pattern = replacements.keys().copied().collect::<Vec<_>>();

    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for (start, part) in text.match_indices(pattern.as_slice()) {
        let ch = unsafe { part.chars().next().unwrap_unchecked() };
        result.push_str(unsafe { text.get_unchecked(last_end..start) });
        result.push_str(unsafe { replacements.get(&ch).unwrap_unchecked() });
        last_end = start + part.len();
    }
    result.push_str(unsafe { text.get_unchecked(last_end..text.len()) });
    result
}

/// GMarkup flavoured XML has defined only 5 entities and doesn't support user-defined entities.
/// See https://gitlab.gnome.org/GNOME/glib/-/blob/353942c6/glib/gmarkup.c#L2189
fn escape_pango_entities(text: &str) -> String {
    let replacements = HashMap::from([
        ('&', "&amp;"),
        ('<', "&lt;"),
        ('>', "&gt;"),
        ('"', "&quot;"),
        ('\'', "&apos;"),
    ]);

    replace_many(text, replacements)
}

/// See https://github.com/qt/qtbase/blob/2e2f1e2/src/gui/text/qtextdocument.cpp#L166
fn escape_qt_text_document(text: &str) -> String {
    let replacements = HashMap::from([
        ('\n', "<br>"),
        ('\t', " "),
        ('<', "&lt;"),
        ('>', "&gt;"),
        ('&', "&amp;"),
    ]);

    let escaped = replace_many(text, replacements);

    format!("<html><body>{}</body></html>", escaped)
}

struct BackendParams<'a> {
    title: &'a str,
    text: &'a str,
    level: MessageLevel,
    ask: bool,
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

    if params.ask {
        backend.command.arg("--yesno");
    } else {
        backend.command.arg("--msgbox");
    }

    let text = escape_qt_text_document(params.text);
    backend.command.arg(text);

    backend.command.arg("--title");
    backend.command.arg(params.title);

    match params.level {
        MessageLevel::Info => backend.command.arg("--icon=dialog-information"),
        MessageLevel::Warning => backend.command.arg("--icon=dialog-warning"),
        MessageLevel::Error => backend.command.arg("--icon=dialog-error"),
    };
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

    backend.command.arg("--width=400");

    if params.ask {
        backend.command.arg("--question");

        // `--icon-name` was renamed to `--icon` at zenity 3.90.0
        match &version {
            Some(v) if *v < (3, 90, 0) => backend.command.arg("--icon-name"),
            _ => backend.command.arg("--icon"),
        };
        match params.level {
            MessageLevel::Info => backend.command.arg("dialog-information"),
            MessageLevel::Warning => backend.command.arg("dialog-warning"),
            MessageLevel::Error => backend.command.arg("dialog-error"),
        };
    } else {
        match params.level {
            MessageLevel::Info => backend.command.arg("--info"),
            MessageLevel::Warning => backend.command.arg("--warning"),
            MessageLevel::Error => backend.command.arg("--error"),
        };
    }

    backend.command.arg("--title");
    backend.command.arg(params.title);

    let text = escape_pango_entities(params.text);
    backend.command.arg("--text");
    backend.command.arg(text);
}

fn init_yad(backend: &mut Backend, params: BackendParams) {
    if params.ask {
        backend.command.arg("--button");
        backend.command.arg("Yes:0");
        backend.command.arg("--button");
        backend.command.arg("No:1");
    } else {
        backend.command.arg("--button");
        backend.command.arg("Ok:0");
    }

    backend.command.arg("--image");
    match params.level {
        MessageLevel::Info => backend.command.arg("dialog-information"),
        MessageLevel::Warning => backend.command.arg("dialog-warning"),
        MessageLevel::Error => backend.command.arg("dialog-error"),
    };

    backend.command.arg("--title");
    backend.command.arg(params.title);

    let text = escape_pango_entities(params.text);
    backend.command.arg("--text");
    backend.command.arg(text);
}
