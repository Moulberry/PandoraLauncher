use bridge::{handle::BackendHandle, message::{MessageToBackend, SyncState}};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, scroll::ScrollableElement, spinner::Spinner, tooltip::Tooltip, v_flex, ActiveTheme as _, Disableable, Icon, IconName, Sizable
};
use schema::backend_config::SyncTarget;

use crate::{entity::DataEntities, ui};

pub struct SyncingPage {
    backend_handle: BackendHandle,
    sync_state: SyncState,
    pending: EnumSet<SyncTarget>,
    loading: EnumSet<SyncTarget>,
    _get_sync_state_task: Task<()>,
}

impl SyncingPage {
    pub fn new(data: &DataEntities, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            backend_handle: data.backend_handle.clone(),
            sync_state: SyncState::default(),
            pending: EnumSet::all(),
            loading: EnumSet::all(),
            _get_sync_state_task: Task::ready(()),
        };

        page.update_sync_state(cx);

        page
    }
}

impl SyncingPage {
    pub fn update_sync_state(&mut self, cx: &mut Context<Self>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        self._get_sync_state_task = cx.spawn(async move |page, cx| {
            let result: SyncState = recv.await.unwrap_or_default();
            let _ = page.update(cx, move |page, cx| {
                page.loading.remove_all(page.pending);
                page.pending = EnumSet::empty();
                page.sync_state = result;
                cx.notify();

                if !page.loading.is_empty() {
                    page.pending = page.loading;
                    page.update_sync_state(cx);
                }
            });
        });

        self.backend_handle.send(MessageToBackend::GetSyncState {
            channel: send,
        });
    }

    pub fn create_entry(&mut self, id: &'static str, label: &'static str, target: SyncTarget, warning: Hsla, info: Hsla, cx: &mut Context<Self>) -> Div {
        let synced_count = self.sync_state.synced[target];
        let cannot_sync_count = self.sync_state.cannot_sync[target];
        let enabled = self.sync_state.want_sync.contains(target);
        let disabled = !enabled && cannot_sync_count > 0;

        let backend_handle = self.backend_handle.clone();
        let checkbox = Checkbox::new(id)
            .label(label)
            .disabled(disabled)
            .checked(enabled)
            .when(disabled, |this| this.tooltip(move |window, cx| {
                Tooltip::new(format!("{} instance(s) already contain a '{}' folder. Please safely backup and remove the folders to enable syncing", cannot_sync_count, target.get_folder().unwrap_or("???"))).build(window, cx)
            }))
            .on_click(cx.listener(move |page, value, _, cx| {
            backend_handle.send(MessageToBackend::SetSyncing {
                target,
                value: *value,
            });

            page.loading.insert(target);
            if page.pending.is_empty() {
                page.pending.insert(target);
                page.update_sync_state(cx);
            }
        }));

        let mut base = h_flex().line_height(relative(1.0)).gap_2p5().child(checkbox);

        if self.loading.contains(target) {
            base = base.child(Spinner::new());
        } else {
            if (enabled || synced_count > 0) && target.get_folder().is_some() {
                base = base.child(h_flex().gap_1().flex_shrink().text_color(info)
                    .child(format!("({}/{} folders synced)", synced_count, self.sync_state.total))
                );
            }
            if enabled && cannot_sync_count > 0 {
                base = base.child(h_flex().gap_1().flex_shrink().text_color(warning)
                    .child(Icon::default().path("icons/triangle-alert.svg"))
                    .child(format!("{}/{} instances are unable to be synced!", cannot_sync_count, self.sync_state.total))
                );
            }
        }


        base
    }
}

impl Render for SyncingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading == EnumSet::all() {
            let content = v_flex().size_full().p_3().gap_3()
                .child("These options allow for syncing various files/folders across instances")
                .child(Spinner::new().with_size(gpui_component::Size::Large));
            return ui::page(cx, h_flex().gap_8().child("Syncing")).child(content).overflow_y_scrollbar();
        }

        let sync_folder = self.sync_state.sync_folder.clone();

        let warning = cx.theme().red;
        let info = cx.theme().blue;
        let content = v_flex().size_full().p_3().gap_3()
            .child("These options allow for syncing various files/folders across instances")
            .when_some(sync_folder, |this, sync_folder| {
                this.child(Button::new("open").info().icon(IconName::FolderOpen).label("Open synced folders directory").on_click(move |_, window, cx| {
                    crate::open_folder(&sync_folder, window, cx);
                }).w_72())
            })
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child("Files"))
            .child(self.create_entry("options", "Sync options.txt", SyncTarget::Options, warning, info, cx))
            .child(self.create_entry("servers", "Sync servers.dat", SyncTarget::Servers, warning, info, cx))
            .child(self.create_entry("commands", "Sync command_history.txt", SyncTarget::Commands, warning, info, cx))
            .child(self.create_entry("hotbars", "Sync hotbar.nbt", SyncTarget::Hotbars, warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child("Folders"))
            .child(self.create_entry("saves", "Sync saves folder", SyncTarget::Saves, warning, info, cx))
            .child(self.create_entry("config", "Sync config folder", SyncTarget::Config, warning, info, cx))
            .child(self.create_entry("screenshots", "Sync screenshots folder", SyncTarget::Screenshots, warning, info, cx))
            .child(self.create_entry("resourcepacks", "Sync resourcepacks folder", SyncTarget::Resourcepacks, warning, info, cx))
            .child(self.create_entry("shaderpacks", "Sync shaderpacks folder", SyncTarget::Shaderpacks, warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child("Mods"))
            .child(self.create_entry("flashback", "Sync Flashback (flashback) folder", SyncTarget::Flashback, warning, info, cx))
            .child(self.create_entry("dh", "Sync Distant Horizons (Distant_Horizons_server_data) folder", SyncTarget::DistantHorizons, warning, info, cx))
            .child(self.create_entry("voxy", "Sync Voxy (.voxy) folder", SyncTarget::Voxy, warning, info, cx))
            .child(self.create_entry("xaero", "Sync Xaero's Minimap (xaero) folder", SyncTarget::XaerosMinimap, warning, info, cx))
            .child(self.create_entry("bobby", "Sync Bobby (.bobby) folder", SyncTarget::Bobby, warning, info, cx));

        ui::page(cx, h_flex().gap_8().child("Syncing")).child(content).overflow_y_scrollbar()
    }
}
