//! Interact with the clipboard.
//!
//! The portal is mostly meant to be used along with
//! [`RemoteDesktop`](crate::desktop::remote_desktop::RemoteDesktop) or
//! [`InputCapture`](crate::desktop::input_capture::InputCapture).

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedFd, OwnedObjectPath, Type, as_value, as_value::optional};

use super::{Session, SessionPortal};
use crate::{Result, proxy::Proxy};

/// A session that is compatible with the Clipboard portal
pub trait IsClipboardSession: SessionPortal {}

#[derive(Debug, Type, Serialize, Default)]
#[zvariant(signature = "dict")]
/// Specified options for a [`Clipboard::set_selection`] request.
pub struct SetSelectionOptions<'a> {
    #[serde(borrow)]
    #[serde(with = "as_value", skip_serializing_if = "<[_]>::is_empty")]
    mime_types: &'a [&'a str],
}

impl<'a> SetSelectionOptions<'a> {
    /// Sets the mime types for the clipboard selection.
    pub fn set_mime_types(mut self, mime_types: &'a [&'a str]) -> Self {
        self.mime_types = mime_types;
        self
    }
}

#[derive(Debug, Type, Serialize, Default)]
#[zvariant(signature = "dict")]
/// Specified options for a [`Clipboard::request`] request.
pub struct RequestClipboardOptions {}

#[derive(Debug, Type, Deserialize, Default)]
#[zvariant(signature = "dict")]
/// The details of a new clipboard selection.
pub struct SelectionOwnerChanged {
    #[serde(default, with = "as_value")]
    mime_types: Vec<String>,
    #[serde(default, with = "optional")]
    session_is_owner: Option<bool>,
}

impl SelectionOwnerChanged {
    /// Whether the session is the owner of the clipboard selection or not.
    pub fn session_is_owner(&self) -> Option<bool> {
        self.session_is_owner
    }

    /// A list of mime types the new clipboard has content for.
    pub fn mime_types(&self) -> &[String] {
        &self.mime_types
    }
}

#[doc(alias = "org.freedesktop.portal.Clipboard")]
/// Wrapper of the DBus interface: [`org.freedesktop.portal.Clipboard`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html).
pub struct Clipboard(Proxy<'static>);

impl Clipboard {
    /// Create a new instance of [`Clipboard`].
    pub async fn new() -> Result<Self> {
        Ok(Self(
            Proxy::new_desktop("org.freedesktop.portal.Clipboard").await?,
        ))
    }

    /// Create a new instance of [`Clipboard`].
    pub async fn with_connection(connection: zbus::Connection) -> Result<Self> {
        Ok(Self(
            Proxy::new_desktop_with_connection(connection, "org.freedesktop.portal.Clipboard")
                .await?,
        ))
    }

    /// Returns the version of the portal interface.
    pub fn version(&self) -> u32 {
        self.0.version()
    }

    /// # Specifications
    ///
    /// See also [`RequestClipboard`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-requestclipboard).
    #[doc(alias = "RequestClipboard")]
    pub async fn request<T: IsClipboardSession>(
        &self,
        session: &Session<T>,
        options: RequestClipboardOptions,
    ) -> Result<()> {
        self.0
            .call_method("RequestClipboard", &(session, options))
            .await?;
        Ok(())
    }

    /// # Specifications
    ///
    /// See also [`SetSelection`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-setselection).
    #[doc(alias = "SetSelection")]
    pub async fn set_selection<'a, T: IsClipboardSession>(
        &self,
        session: &Session<T>,
        options: SetSelectionOptions<'a>,
    ) -> Result<()> {
        self.0
            .call::<()>("SetSelection", &(session, options))
            .await?;

        Ok(())
    }

    /// # Specifications
    ///
    /// See also [`SelectionWrite`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-selectionwrite).
    #[doc(alias = "SelectionWrite")]
    pub async fn selection_write<T: IsClipboardSession>(
        &self,
        session: &Session<T>,
        serial: u32,
    ) -> Result<OwnedFd> {
        let fd = self
            .0
            .call::<OwnedFd>("SelectionWrite", &(session, serial))
            .await?;
        Ok(fd)
    }

    /// # Specifications
    ///
    /// See also [`SelectionWriteDone`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-selectionwritedone).
    #[doc(alias = "SelectionWriteDone")]
    pub async fn selection_write_done<T: IsClipboardSession>(
        &self,
        session: &Session<T>,
        serial: u32,
        success: bool,
    ) -> Result<()> {
        self.0
            .call("SelectionWriteDone", &(session, serial, success))
            .await
    }

    /// # Specifications
    ///
    /// See also [`SelectionRead`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-selectionread).
    #[doc(alias = "SelectionRead")]
    pub async fn selection_read<T: IsClipboardSession>(
        &self,
        session: &Session<T>,
        mime_type: &str,
    ) -> Result<OwnedFd> {
        let fd = self
            .0
            .call::<OwnedFd>("SelectionRead", &(session, mime_type))
            .await?;
        Ok(fd)
    }

    /// Notifies the session that the clipboard selection has changed.
    /// # Specifications
    ///
    /// See also [`SelectionOwnerChanged`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-selectionownerchanged).
    #[doc(alias = "SelectionOwnerChanged")]
    pub async fn receive_selection_owner_changed<T: IsClipboardSession>(
        &self,
    ) -> Result<impl Stream<Item = (Session<T>, SelectionOwnerChanged)>> {
        let connection = self.connection().clone();
        Ok(self
            .0
            .signal::<(OwnedObjectPath, SelectionOwnerChanged)>("SelectionOwnerChanged")
            .await?
            .filter_map(move |(p, o)| {
                let connection = connection.clone();
                async move {
                    Session::with_connection(connection, p)
                        .await
                        .map(|s| (s, o))
                        .ok()
                }
            }))
    }

    /// # Specifications
    ///
    /// See also [`SelectionTransfer`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html#org-freedesktop-portal-clipboard-selectiontransfer).
    #[doc(alias = "SelectionTransfer")]
    pub async fn receive_selection_transfer<T: IsClipboardSession>(
        &self,
    ) -> Result<impl Stream<Item = (Session<T>, String, u32)>> {
        let connection = self.connection().clone();
        Ok(self
            .0
            .signal::<(OwnedObjectPath, String, u32)>("SelectionTransfer")
            .await?
            .filter_map(move |(p, mime_type, serial)| {
                let connection = connection.clone();
                async move {
                    Session::with_connection(connection, p)
                        .await
                        .map(|session| (session, mime_type, serial))
                        .ok()
                }
            }))
    }
}

impl std::ops::Deref for Clipboard {
    type Target = zbus::Proxy<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
