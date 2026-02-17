use bridge::{handle::BackendHandle, message::{MessageToBackend, SyncState}};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, scroll::ScrollableElement, spinner::Spinner, tooltip::Tooltip, v_flex, ActiveTheme as _, Disableable, Icon, IconName, Sizable
};
use schema::backend_config::SyncTarget;

use crate::{entity::DataEntities, ts, ui};

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

    pub fn create_entry(&mut self, id: &'static str, label: SharedString, target: SyncTarget, warning: Hsla, info: Hsla, cx: &mut Context<Self>) -> Div {
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
                Tooltip::new(ts!("instance.sync.already_exists", num = cannot_sync_count, name = target.get_folder().unwrap_or("???"))).build(window, cx)
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
                    .child(ts!("instance.sync.folders_count", num1 = synced_count, num2 = self.sync_state.total))
                );
            }
            if enabled && cannot_sync_count > 0 {
                base = base.child(h_flex().gap_1().flex_shrink().text_color(warning)
                    .child(Icon::default().path("icons/triangle-alert.svg"))
                    .child(ts!("instance.sync.unable_count", num1 = cannot_sync_count, num2 = self.sync_state.total))
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
                .child(ts!("instance.sync.description"))
                .child(Spinner::new().with_size(gpui_component::Size::Large));
            return ui::page(cx, h_flex().gap_8().child(ts!("instance.sync.label"))).child(content).overflow_y_scrollbar();
        }

        let sync_folder = self.sync_state.sync_folder.clone();

        let warning = cx.theme().red;
        let info = cx.theme().blue;
        let content = v_flex().size_full().p_3().gap_3()
            .child(ts!("instance.sync.description"))
            .when_some(sync_folder, |this, sync_folder| {
                this.child(Button::new("open").info().icon(IconName::FolderOpen).label(ts!("instance.sync.open_folder")).on_click(move |_, window, cx| {
                    crate::open_folder(&sync_folder, window, cx);
                }).w_72())
            })
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(ts!("instance.sync.files")))
            .child(self.create_entry("options", ts!("instance.sync.targets.options"), SyncTarget::Options, warning, info, cx))
            .child(self.create_entry("servers", ts!("instance.sync.targets.servers"), SyncTarget::Servers, warning, info, cx))
            .child(self.create_entry("commands", ts!("instance.sync.targets.commands"), SyncTarget::Commands, warning, info, cx))
            .child(self.create_entry("hotbars", ts!("instance.sync.targets.hotbars"), SyncTarget::Hotbars, warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(ts!("instance.sync.folders")))
            .child(self.create_entry("saves", ts!("instance.sync.targets.saves"), SyncTarget::Saves, warning, info, cx))
            .child(self.create_entry("config", ts!("instance.sync.targets.config"), SyncTarget::Config, warning, info, cx))
            .child(self.create_entry("screenshots", ts!("instance.sync.targets.screenshots"), SyncTarget::Screenshots, warning, info, cx))
            .child(self.create_entry("resourcepacks", ts!("instance.sync.targets.resourcepacks"), SyncTarget::Resourcepacks, warning, info, cx))
            .child(self.create_entry("shaderpacks", ts!("instance.sync.targets.shaderpacks"), SyncTarget::Shaderpacks, warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(ts!("instance.sync.mods")))
            .child(self.create_entry("flashback", ts!("instance.sync.targets.flashback"), SyncTarget::Flashback, warning, info, cx))
            .child(self.create_entry("dh", ts!("instance.sync.targets.dh"), SyncTarget::DistantHorizons, warning, info, cx))
            .child(self.create_entry("voxy", ts!("instance.sync.targets.voxy"), SyncTarget::Voxy, warning, info, cx))
            .child(self.create_entry("xaero", ts!("instance.sync.targets.xaero"), SyncTarget::XaerosMinimap, warning, info, cx))
            .child(self.create_entry("bobby", ts!("instance.sync.targets.bobby"), SyncTarget::Bobby, warning, info, cx))
            .child(self.create_entry("litematic", ts!("instance.sync.targets.litematic"), SyncTarget::Litematic, warning, info, cx));

        ui::page(cx, h_flex().gap_8().child(ts!("instance.sync.label"))).child(content).overflow_y_scrollbar()
    }
}
