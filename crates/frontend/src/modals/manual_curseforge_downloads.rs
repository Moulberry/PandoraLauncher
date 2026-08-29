use std::{collections::HashSet, path::PathBuf};

use bridge::{manual_download::{ManualCurseforgeDownload, ManualCurseforgeDownloadRequest, ManualCurseforgeDownloadStart}, message::MessageToBackend};
use directories::BaseDirs;
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, WindowExt, button::{Button, ButtonVariants}, dialog::Dialog, h_flex, scroll::ScrollableElement, v_flex};

use crate::entity::DataEntities;

struct ManualCurseforgeDownloadsDialog {
    data: DataEntities,
    session_id: uuid::Uuid,
    files: std::sync::Arc<[ManualCurseforgeDownload]>,
    completed: HashSet<[u8; 20]>,
    directory: PathBuf,
    started: bool,
}

pub fn open(request: ManualCurseforgeDownloadRequest, data: DataEntities, window: &mut Window, cx: &mut App) {
    let ManualCurseforgeDownloadRequest { session_id, files, mut progress, completion } = request;
    let directory = BaseDirs::new().map(|dirs| dirs.home_dir().join("Downloads")).unwrap_or_default();
    let dialog = cx.new(|_| ManualCurseforgeDownloadsDialog { data, session_id, files, completed: Default::default(), directory, started: false });
    let progress_dialog = dialog.clone();
    window.open_dialog(cx, move |modal, window, cx| dialog.update(cx, |this, cx| this.render(modal, window, cx)));

    let window_handle = window.window_handle();
    window.spawn(cx, async move |cx| {
        while let Some(hash) = progress.recv().await {
            progress_dialog.update(cx, |this, cx| {
                this.completed.insert(hash);
                cx.notify();
            });
        }
    }).detach();
    window.spawn(cx, async move |cx| {
        _ = completion.await;
        _ = cx.update_window(window_handle, |_, window, cx| window.close_dialog(cx));
    }).detach();
}

impl ManualCurseforgeDownloadsDialog {
    fn start_watching(&mut self, cx: &mut Context<Self>) {
        if self.started { return; }
        self.started = true;
        self.data.backend_handle.send(MessageToBackend::StartManualCurseforgeDownloads {
            request: ManualCurseforgeDownloadStart { session_id: self.session_id, directory: self.directory.clone() },
        });
        cx.notify();
    }

    fn open_all(&mut self, cx: &mut Context<Self>) {
        self.start_watching(cx);
        for file in self.files.iter() {
            if let Err(err) = open::that_detached(file.page_url.as_ref()) {
                log::error!("Failed to open manual CurseForge download URL: {err}");
            }
        }
    }

    fn check_downloads(&mut self) {
        if !self.started { return; }
        self.data.backend_handle.send(MessageToBackend::CheckManualCurseforgeDownloads {
            session_id: self.session_id,
        });
    }

    fn render(&mut self, modal: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        let files = self.files.clone();
        let directory = self.directory.display().to_string();
        let max_list_height = window.viewport_size().height * 0.55;
        let list_height = px(files.len() as f32 * 72.0).min(max_list_height);
        let select_folder = cx.listener(|_this, _: &ClickEvent, _, cx| {
            let receiver = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(t::instance::content::manual_curseforge_downloads::choose_folder().into()) });
            cx.spawn(async move |this, cx| {
                let Ok(Ok(Some(mut paths))) = receiver.await else { return; };
                let Some(path) = paths.pop() else { return; };
                _ = this.update(cx, |this, cx| { this.directory = path; cx.notify(); });
            }).detach();
        });
        let open_all = cx.listener(|this, _: &ClickEvent, _, cx| this.open_all(cx));
        let check_downloads = cx.listener(|this, _: &ClickEvent, _, _| this.check_downloads());
        let cancel = self.data.backend_handle.clone();
        let session_id = self.session_id;
        modal.title(t::instance::content::manual_curseforge_downloads::title())
            .child(v_flex().gap_2().w_full().min_w_0().child(div().w_full().whitespace_normal().child(t::instance::content::manual_curseforge_downloads::description()))
                .child(h_flex().w_full().gap_2()
                    .child(div().flex_1().min_w_0().text_ellipsis().child(t::instance::content::manual_curseforge_downloads::downloads_folder(&directory)))
                    .child(Button::new("choose-folder").flex_shrink_0().label(t::instance::content::manual_curseforge_downloads::change_folder()).disabled(self.started).on_click(select_folder)))
                .child(v_flex().h(list_height).flex_shrink_0().overflow_y_scrollbar().gap_1().children(files.iter().map(|file| {
                    let completed = self.completed.contains(&file.sha1);
                    let trailing = if completed {
                        div().flex_shrink_0().text_color(cx.theme().success).child(format!("{} ✓", t::instance::content::manual_curseforge_downloads::downloaded())).into_any_element()
                    } else {
                        let url = file.page_url.clone();
                        let open = cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.start_watching(cx);
                            cx.open_url(&url);
                        });
                        Button::new(SharedString::new(format!("open-{}", file.file_id))).flex_shrink_0().label(t::instance::content::manual_curseforge_downloads::open()).on_click(open).into_any_element()
                    };
                    h_flex().w_full().gap_2().p_2().rounded(cx.theme().radius_lg).bg(cx.theme().background)
                        .child(div().flex_1().min_w_0().whitespace_normal().child(format!("{} — {}", file.name, file.filename)))
                        .child(trailing)
                }))))
            .footer(h_flex().justify_end().gap_2()
                .child(Button::new("cancel").label(t::common::cancel()).on_click(move |_, _, _| { cancel.send(MessageToBackend::CancelManualCurseforgeDownloads { session_id }); }))
                .child(Button::new("check-downloads").disabled(!self.started).label(t::instance::content::manual_curseforge_downloads::check_downloads()).on_click(check_downloads))
                .child(Button::new("open-all").success().disabled(self.started).label(if self.started { t::instance::content::manual_curseforge_downloads::watching() } else { t::instance::content::manual_curseforge_downloads::open_all() }).on_click(open_all)))
    }
}
