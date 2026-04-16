use std::{collections::HashSet, sync::Arc};

use bridge::{handle::BackendHandle, message::{MessageToBackend, SyncState}, safe_path::SafePath};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputState}, spinner::Spinner, v_flex, ActiveTheme as _, Disableable, Sizable
};
use once_cell::sync::Lazy;
use rustc_hash::FxHashSet;

use crate::{entity::DataEntities, icon::PandoraIcon, pages::page::Page};

pub struct SyncingPage {
    backend_handle: BackendHandle,
    sync_state: Option<SyncState>,
    pending: FxHashSet<Arc<str>>,
    loading: FxHashSet<Arc<str>>,
    custom_input_state: Entity<InputState>,
    _get_sync_state_task: Task<()>,
}

impl SyncingPage {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            backend_handle: data.backend_handle.clone(),
            sync_state: None,
            pending: FxHashSet::default(),
            loading: FxHashSet::default(),
            custom_input_state: cx.new(|cx| InputState::new(window, cx)),
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
            let Ok(result): Result<SyncState, _> = recv.await else {
                return;
            };
            let _ = page.update(cx, move |page, cx| {
                page.loading.retain(|loading| !page.pending.contains(loading));
                page.pending = FxHashSet::default();
                page.sync_state = Some(result);
                cx.notify();

                if !page.loading.is_empty() {
                    page.pending = page.loading.clone();
                    page.update_sync_state(cx);
                }
            });
        });

        self.backend_handle.send(MessageToBackend::GetSyncState {
            channel: send,
        });
    }

    pub fn create_entry(&self, sync_state: &SyncState, name: Arc<str>, is_file: bool, label: SharedString, warning: Hsla, info: Hsla, cx: &mut Context<Self>) -> Div {
        let synced_count;
        let cannot_sync_count;
        let enabled;
        if let Some(sync_target_state) = sync_state.targets.get(&name) && sync_target_state.is_file == is_file {
            synced_count = sync_target_state.sync_count;
            cannot_sync_count = sync_target_state.cannot_sync_count;
            enabled = sync_target_state.enabled;
        } else {
            synced_count = 0;
            cannot_sync_count = 0;
            enabled = false;
        }
        let disabled = !enabled && cannot_sync_count > 0;
        let is_loading = self.loading.contains(&name);

        let disable_tooltip = t::instance::sync::already_exists(cannot_sync_count, &name);
        let backend_handle = self.backend_handle.clone();
        let checkbox = Checkbox::new(name.clone())
            .label(label)
            .disabled(disabled)
            .checked(enabled)
            .when(disabled, |this| this.tooltip(disable_tooltip))
            .on_click(cx.listener(move |page, value, _, cx| {

            backend_handle.send(MessageToBackend::SetSyncing {
                target: name.clone(),
                is_file,
                value: *value,
            });

            page.loading.insert(name.clone());
            if page.pending.is_empty() {
                page.pending.insert(name.clone());
                page.update_sync_state(cx);
            }
        }));

        let mut base = h_flex().line_height(relative(1.0)).gap_2p5().child(checkbox);

        if is_loading {
            base = base.child(Spinner::new());
        } else {
            if (enabled || synced_count > 0) && !is_file {
                base = base.child(h_flex().gap_1().flex_shrink().text_color(info)
                    .child(t::instance::sync::folders_count(synced_count, sync_state.total_count))
                );
            }
            if enabled && cannot_sync_count > 0 {
                base = base.child(h_flex().gap_1().flex_shrink().text_color(warning)
                    .child(PandoraIcon::TriangleAlert)
                    .child(t::instance::sync::unable_count(cannot_sync_count, sync_state.total_count))
                );
            }
        }


        base
    }
}

impl Page for SyncingPage {
    fn controls(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }

    fn scrollable(&self, _cx: &App) -> bool {
        true
    }
}

impl Render for SyncingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(sync_state) = &self.sync_state else {
            let content = v_flex().size_full().p_3().gap_3()
                .child(t::instance::sync::description())
                .child(Spinner::new().with_size(gpui_component::Size::Large));

            return content;
        };

        let sync_folder = sync_state.sync_folder.clone();

        let warning = cx.theme().red;
        let info = cx.theme().blue;
        let content = v_flex().size_full().p_3().gap_3()
            .child(t::instance::sync::description())
            .child(Button::new("open").info().icon(PandoraIcon::FolderOpen).label(t::instance::sync::open_folder()).on_click(move |_, window, cx| {
                crate::open_folder(&sync_folder, window, cx);
            }).w_72())
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(t::instance::sync::files()))
            .child(self.create_entry(sync_state, "options.txt".into(), true,  t::instance::sync::targets::options().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "servers.dat".into(), true, t::instance::sync::targets::servers().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "command_history.txt".into(), true, t::instance::sync::targets::commands().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "hotbar.nbt".into(), true, t::instance::sync::targets::hotbars().into(), warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(t::instance::sync::folders()))
            .child(self.create_entry(sync_state, "saves".into(), false, t::instance::sync::targets::saves().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "config".into(), false, t::instance::sync::targets::config().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "screenshots".into(), false, t::instance::sync::targets::screenshots().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "resourcepacks".into(), false, t::instance::sync::targets::resourcepacks().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "shaderpacks".into(), false, t::instance::sync::targets::shaderpacks().into(), warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(t::instance::sync::mods()))
            .child(self.create_entry(sync_state, "flashback".into(), false, t::instance::sync::targets::flashback().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "Distant_Horizons_server_data".into(), false, t::instance::sync::targets::dh().into(), warning, info, cx))
            .child(self.create_entry(sync_state, ".voxy".into(), false, t::instance::sync::targets::voxy().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "xaero".into(), false, t::instance::sync::targets::xaero().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "journeymap".into(), false, t::instance::sync::targets::journeymap().into(), warning, info, cx))
            .child(self.create_entry(sync_state, ".bobby".into(), false, t::instance::sync::targets::bobby().into(), warning, info, cx))
            .child(self.create_entry(sync_state, "schematics".into(), false, t::instance::sync::targets::litematic().into(), warning, info, cx))
            .child(div().border_b_1().border_color(cx.theme().border).text_lg().child(t::instance::sync::custom()))
            .children(sync_state.targets.iter().filter_map(|(name, state)| {
                if !state.enabled || NAMED_SYNC_TARGETS.contains(&**name) {
                    return None;
                }
                let label = if state.is_file {
                    t::instance::sync::sync_name_file(&name)
                } else {
                    t::instance::sync::sync_name_folder(&name)
                };
                Some(self.create_entry(sync_state, name.clone(), state.is_file, label.into(), warning, info, cx))
            }))
            .child(h_flex()
                .w_full()
                .gap_2()
                .child(Input::new(&self.custom_input_state).max_w_128())
                .child(Button::new("custom_file").label(t::instance::sync::sync_file()).on_click(cx.listener(|page, _, window, cx| {
                    let input = page.custom_input_state.read(cx).value();
                    let input = input.as_str().trim_ascii();
                    if SafePath::new(input).is_some() {
                        let name: Arc<str> = input.into();
                        page.backend_handle.send(MessageToBackend::SetSyncing {
                            target: name.clone(),
                            is_file: true,
                            value: true,
                        });

                        page.loading.insert(name.clone());
                        if page.pending.is_empty() {
                            page.pending.insert(name.clone());
                            page.update_sync_state(cx);
                        }

                        page.custom_input_state.update(cx, |state, cx| state.set_value("", window, cx));
                    }
                })))
                .child(Button::new("custom_folder").label(t::instance::sync::sync_folder()).on_click(cx.listener(|page, _, window, cx| {
                    let input = page.custom_input_state.read(cx).value();
                    let input = input.as_str().trim_ascii();
                    if SafePath::new(input).is_some() {
                        let name: Arc<str> = input.into();
                        page.backend_handle.send(MessageToBackend::SetSyncing {
                            target: name.clone(),
                            is_file: false,
                            value: true,
                        });

                        page.loading.insert(name.clone());
                        if page.pending.is_empty() {
                            page.pending.insert(name.clone());
                            page.update_sync_state(cx);
                        }

                        page.custom_input_state.update(cx, |state, cx| state.set_value("", window, cx));
                    }
                }))));

        content
    }
}

static NAMED_SYNC_TARGETS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "options.txt",
        "servers.dat",
        "command_history.txt",
        "hotbar.nbt",
        "saves",
        "config",
        "screenshots",
        "resourcepacks",
        "shaderpacks",
        "flashback",
        "Distant_Horizons_server_data",
        ".voxy",
        "xaero",
        "journeymap",
        ".bobby",
        "schematics"
    ])
});
