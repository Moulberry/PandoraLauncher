use std::path::PathBuf;

use bridge::{manual_download::{ManualCurseforgeDownload, ManualCurseforgeDownloadRequest, ManualCurseforgeDownloadStart}, message::MessageToBackend};
use directories::BaseDirs;
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, WindowExt, button::{Button, ButtonVariants}, dialog::Dialog, h_flex, v_flex};

use crate::entity::DataEntities;

struct ManualCurseforgeDownloadsDialog {
    data: DataEntities,
    session_id: uuid::Uuid,
    files: std::sync::Arc<[ManualCurseforgeDownload]>,
    directory: PathBuf,
    started: bool,
}

pub fn open(request: ManualCurseforgeDownloadRequest, data: DataEntities, window: &mut Window, cx: &mut App) {
    let ManualCurseforgeDownloadRequest { session_id, files, completion } = request;
    let directory = BaseDirs::new().map(|dirs| dirs.home_dir().join("Downloads")).unwrap_or_default();
    let dialog = cx.new(|_| ManualCurseforgeDownloadsDialog { data, session_id, files, directory, started: false });
    window.open_dialog(cx, move |modal, window, cx| dialog.update(cx, |this, cx| this.render(modal, window, cx)));

    let window_handle = window.window_handle();
    window.spawn(cx, async move |cx| {
        _ = completion.await;
        _ = cx.update_window(window_handle, |_, window, cx| window.close_dialog(cx));
    }).detach();
}

impl ManualCurseforgeDownloadsDialog {
    fn start(&mut self, cx: &mut Context<Self>) {
        if self.started { return; }
        self.started = true;
        self.data.backend_handle.send(MessageToBackend::StartManualCurseforgeDownloads {
            request: ManualCurseforgeDownloadStart { session_id: self.session_id, directory: self.directory.clone() },
        });
        for file in self.files.iter() { cx.open_url(&file.page_url); }
        cx.notify();
    }

    fn render(&mut self, modal: Dialog, _window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        let files = self.files.clone();
        let directory = self.directory.display().to_string();
        let select_folder = cx.listener(|_this, _: &ClickEvent, _, cx| {
            let receiver = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose downloads folder".into()) });
            cx.spawn(async move |this, cx| {
                let Ok(Ok(Some(mut paths))) = receiver.await else { return; };
                let Some(path) = paths.pop() else { return; };
                _ = this.update(cx, |this, cx| { this.directory = path; cx.notify(); });
            }).detach();
        });
        let start = cx.listener(|this, _: &ClickEvent, _, cx| this.start(cx));
        let cancel = self.data.backend_handle.clone();
        let session_id = self.session_id;
        modal.title("Manual CurseForge downloads")
            .child(v_flex().gap_2().w(px(620.)).child("Some mod authors require their files to be downloaded through CurseForge. Pandora will verify each completed download automatically.")
                .child(h_flex().justify_between().child(format!("Downloads folder: {directory}")).child(Button::new("choose-folder").label("Change folder").disabled(self.started).on_click(select_folder)))
                .child(v_flex().gap_1().children(files.iter().map(|file| {
                    h_flex().justify_between().p_2().rounded(cx.theme().radius_lg).bg(cx.theme().background)
                        .child(format!("{} — {}", file.name, file.filename))
                        .child(Button::new(SharedString::new(format!("open-{}", file.file_id))).label("Open").on_click({ let url = file.page_url.clone(); move |_, _, cx| cx.open_url(&url) }))
                }))))
            .footer(h_flex().justify_end().gap_2()
                .child(Button::new("cancel").label("Cancel").on_click(move |_, _, _| { cancel.send(MessageToBackend::CancelManualCurseforgeDownloads { session_id }); }))
                .child(Button::new("open-all").success().disabled(self.started).label(if self.started { "Watching downloads…" } else { "Open all" }).on_click(start)))
    }
}
