use std::{path::Path, sync::Arc};

use bridge::{
    handle::BackendHandle,
    install::ContentInstall,
    instance::{InstanceID, InstanceContentID},
    message::{MessageToBackend, QuickPlayLaunch},
    modal_action::ModalAction,
};
use gpui::{prelude::*, *};
use gpui_component::{Root, StyledExt, Theme, breadcrumb::Breadcrumb, scroll::{ScrollableElement, ScrollbarAxis}, v_flex};
use parking_lot::RwLock;

use crate::{CloseWindow, MAIN_FONT, entity::DataEntities, modals, ts, ui::{LauncherUI, PageType}};

pub struct LauncherRootGlobal {
    pub root: Entity<LauncherRoot>,
}

impl Global for LauncherRootGlobal {}

pub struct LauncherRoot {
    pub ui: Entity<LauncherUI>,
    pub panic_message: Arc<RwLock<Option<String>>>,
    pub deadlock_message: Arc<RwLock<Option<String>>>,
    pub backend_handle: BackendHandle,
    focus_handle: FocusHandle,
}

impl LauncherRoot {
    pub fn new(
        data: &DataEntities,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let launcher_ui = cx.new(|cx| LauncherUI::new(data, window, cx));

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        Self {
            ui: launcher_ui,
            panic_message: data.panic_messages.panic_message.clone(),
            deadlock_message: data.panic_messages.deadlock_message.clone(),
            backend_handle: data.backend_handle.clone(),
            focus_handle,
        }
    }
}

impl Render for LauncherRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(message) = &*self.deadlock_message.read() {
            let purple = Hsla {
                h: 0.8333333333,
                s: 1.,
                l: 0.25,
                a: 1.,
            };
            return v_flex().size_full().bg(purple).child(message.clone()).overflow_y_scrollbar().into_any_element();
        }
        if let Some(message) = &*self.panic_message.read() {
            return v_flex().size_full().bg(gpui::blue()).child(message.clone()).overflow_y_scrollbar().into_any_element();
        }
        if self.backend_handle.is_closed() {
            return v_flex().size_full().bg(gpui::red()).child(ts!("system.backend_shutdown")).into_any_element();
        }

        let has_csd_titlebar = matches!(window.window_decorations(), Decorations::Client { .. });
        Theme::global_mut(cx).sheet.margin_top = if has_csd_titlebar {
            gpui_component::TITLE_BAR_HEIGHT
        } else {
            Pixels::ZERO
        };

        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        v_flex()
            .size_full()
            .font_family(MAIN_FONT)
            .when(has_csd_titlebar, |this| {
                this.child(gpui_component::TitleBar::new().child(ts!("common.app_name")))
            })
            .child(self.ui.clone())
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
            .track_focus(&self.focus_handle)
            .on_action(|_: &CloseWindow, window, _| {
                window.remove_window();
            })
            .into_any_element()
    }
}

pub fn start_new_account_login(
    backend_handle: &BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let modal_action = ModalAction::default();

    backend_handle.send(MessageToBackend::AddNewAccount {
        modal_action: modal_action.clone(),
    });

    let title = ts!("account.add.title");
    modals::generic::show_modal(window, cx, title, ts!("account.add.error"), modal_action);
}

pub fn start_instance(
    id: InstanceID,
    name: SharedString,
    quick_play: Option<QuickPlayLaunch>,
    backend_handle: &BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let modal_action = ModalAction::default();

    backend_handle.send(MessageToBackend::StartInstance {
        id,
        quick_play,
        modal_action: modal_action.clone(),
    });

    let title: SharedString = ts!("instance.start.title", name = name);
    modals::generic::show_modal(window, cx, title, ts!("instance.start.error"), modal_action);
}

pub fn start_install(
    content_install: ContentInstall,
    backend_handle: &BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let modal_action = ModalAction::default();

    backend_handle.send(MessageToBackend::InstallContent {
        content: content_install.clone(),
        modal_action: modal_action.clone(),
    });

    modals::generic::show_notification(window, cx, ts!("instance.content.install.error"), modal_action);
}

pub fn start_update_check(
    instance: InstanceID,
    backend_handle: &BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let modal_action = ModalAction::default();

    backend_handle.send(MessageToBackend::UpdateCheck {
        instance,
        modal_action: modal_action.clone(),
    });

    let title: SharedString = ts!("instance.content.update.check.title");
    modals::generic::show_modal(window, cx, title, ts!("instance.content.update.check.error"), modal_action);
}

pub fn update_single_mod(
    instance: InstanceID,
    mod_id: InstanceContentID,
    backend_handle: &BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let modal_action = ModalAction::default();

    backend_handle.send(MessageToBackend::UpdateContent {
        instance,
        content_id: mod_id,
        modal_action: modal_action.clone(),
    });

    modals::generic::show_notification(window, cx, ts!("instance.content.update.download.error"), modal_action);
}

pub fn upload_log_file(
    path: Arc<Path>,
    backend_handle: &BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let modal_action = ModalAction::default();

    backend_handle.send(MessageToBackend::UploadLogFile {
        path,
        modal_action: modal_action.clone(),
    });

    let title: SharedString = ts!("instance.logs.upload.title");
    modals::generic::show_modal(window, cx, title, ts!("instance.logs.upload.error"), modal_action);
}

pub fn switch_page(
    page: PageType,
    breadcrumbs: &[PageType],
    window: &mut Window,
    cx: &mut App,
) {
    cx.update_global::<LauncherRootGlobal, ()>(|global, cx| {
        global.root.update(cx, |launcher_root, cx| {
            launcher_root.ui.update(cx, |ui, cx| {
                ui.switch_page(page, breadcrumbs, window, cx);
            });
        });
    });
}
