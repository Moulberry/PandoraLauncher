use bridge::{handle::BackendHandle, message::{MessageToBackend, SyncState}};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    scroll::ScrollableElement,
    spinner::Spinner,
    tooltip::Tooltip,
    v_flex,
    ActiveTheme as _,
    Disableable,
    Icon,
    IconName,
    Sizable,
    input::{Input, InputState, NumberInput, NumberInputEvent},
};
use schema::backend_config::SyncTarget;
use schema::instance::{InstanceMemoryConfiguration, InstanceJvmFlagsConfiguration, InstanceJvmBinaryConfiguration};
use std::path::Path;

use crate::{entity::DataEntities, ui, InterfaceConfig};

pub struct SyncingPage {
    backend_handle: BackendHandle,
    sync_state: SyncState,
    pending: EnumSet<SyncTarget>,
    loading: EnumSet<SyncTarget>,
    _get_sync_state_task: Task<()>,
    // Global instance override configuration
    global_memory_enabled: bool,
    global_memory_min_input_state: Entity<InputState>,
    global_memory_max_input_state: Entity<InputState>,
    global_jvm_flags_enabled: bool,
    global_jvm_flags_input_state: Entity<InputState>,
    global_jvm_binary_enabled: bool,
    global_jvm_binary_path: Option<std::sync::Arc<Path>>,
    _select_jvm_binary_task: Task<()>,
}

impl SyncingPage {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let interface_config = InterfaceConfig::get(cx);
        
        let global_memory_min_value = interface_config.global_memory
            .as_ref()
            .map(|m| m.min.to_string())
            .unwrap_or_else(|| "1024".to_string());
        let global_memory_max_value = interface_config.global_memory
            .as_ref()
            .map(|m| m.max.to_string())
            .unwrap_or_else(|| "2048".to_string());
        let global_jvm_flags_value = interface_config.global_jvm_flags
            .as_ref()
            .map(|f| f.flags.to_string())
            .unwrap_or_default();
        let global_memory_enabled = interface_config.global_memory.as_ref().map(|m| m.enabled).unwrap_or(false);
        let global_jvm_flags_enabled = interface_config.global_jvm_flags.as_ref().map(|f| f.enabled).unwrap_or(false);
        let global_jvm_binary_enabled = interface_config.global_jvm_binary.as_ref().map(|b| b.enabled).unwrap_or(false);
        let global_jvm_binary_path = interface_config.global_jvm_binary.as_ref().and_then(|b| b.path.clone());
        
        let mut page = Self {
            backend_handle: data.backend_handle.clone(),
            sync_state: SyncState::default(),
            pending: EnumSet::all(),
            loading: EnumSet::all(),
            _get_sync_state_task: Task::ready(()),
            global_memory_enabled,
            global_memory_min_input_state: cx.new(|cx| InputState::new(window, cx).default_value(global_memory_min_value)),
            global_memory_max_input_state: cx.new(|cx| InputState::new(window, cx).default_value(global_memory_max_value)),
            global_jvm_flags_enabled,
            global_jvm_flags_input_state: cx.new(|cx| InputState::new(window, cx).auto_grow(1, 8).default_value(global_jvm_flags_value)),
            global_jvm_binary_enabled,
            global_jvm_binary_path,
            _select_jvm_binary_task: Task::ready(()),
        };

        page.update_sync_state(cx);
        cx.subscribe(&page.global_memory_min_input_state, Self::on_global_memory_changed).detach();
        cx.subscribe(&page.global_memory_max_input_state, Self::on_global_memory_changed).detach();
        cx.subscribe(&page.global_jvm_flags_input_state, Self::on_global_jvm_flags_changed).detach();

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

        self.backend_handle.send(MessageToBackend::GetSyncState { channel: send });
    }

    pub fn create_entry(
        &mut self,
        id: &'static str,
        label: &'static str,
        target: SyncTarget,
        warning: Hsla,
        info: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        let synced_count = self.sync_state.synced[target];
        let cannot_sync_count = self.sync_state.cannot_sync[target];
        let enabled = self.sync_state.want_sync.contains(target);
        let disabled = !enabled && cannot_sync_count > 0;

        let backend_handle = self.backend_handle.clone();
        let checkbox = Checkbox::new(id)
            .label(label)
            .disabled(disabled)
            .checked(enabled)
            .when(disabled, |this| {
                this.tooltip(move |window, cx| {
                    Tooltip::new(format!(
                        "{} instance(s) already contain a '{}' folder. Please safely backup and remove the folders to enable syncing",
                        cannot_sync_count,
                        target.get_folder().unwrap_or("???")
                    ))
                    .build(window, cx)
                })
            })
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
                base = base.child(
                    h_flex()
                        .gap_1()
                        .flex_shrink()
                        .text_color(info)
                        .child(format!("({}/{} folders synced)", synced_count, self.sync_state.total)),
                );
            }
            if enabled && cannot_sync_count > 0 {
                base = base.child(
                    h_flex()
                        .gap_1()
                        .flex_shrink()
                        .text_color(warning)
                        .child(Icon::default().path("icons/triangle-alert.svg"))
                        .child(format!(
                            "{}/{} instances are unable to be synced!",
                            cannot_sync_count,
                            self.sync_state.total
                        )),
                );
            }
        }

        base
    }

    fn on_global_memory_changed(
        &mut self,
        _: Entity<InputState>,
        _event: &gpui_component::input::InputEvent,
        cx: &mut Context<Self>,
    ) {
        if !self.global_memory_enabled {
            return;
        }
        let min = self.global_memory_min_input_state.read(cx).value().parse::<u32>().unwrap_or(0);
        let max = self.global_memory_max_input_state.read(cx).value().parse::<u32>().unwrap_or(0);

        let memory = InstanceMemoryConfiguration { enabled: true, min, max };
        InterfaceConfig::get_mut(cx).global_memory = Some(memory.clone());
        self.send_global_overrides(cx);
    }

    fn on_global_jvm_flags_changed(
        &mut self,
        _: Entity<InputState>,
        _event: &gpui_component::input::InputEvent,
        cx: &mut Context<Self>,
    ) {
        if !self.global_jvm_flags_enabled {
            return;
        }
        let flags = self.global_jvm_flags_input_state.read(cx).value();

        let jvm_flags = if !flags.is_empty() {
            Some(InstanceJvmFlagsConfiguration { enabled: true, flags: flags.into() })
        } else {
            None
        };
        InterfaceConfig::get_mut(cx).global_jvm_flags = jvm_flags;
        self.send_global_overrides(cx);
    }

    fn send_global_overrides(&self, cx: &App) {
        let interface_config = InterfaceConfig::get(cx);
        self.backend_handle.send(MessageToBackend::SetGlobalInstanceOverrides {
            memory_enabled: self.global_memory_enabled,
            memory: interface_config.global_memory.clone(),
            jvm_flags_enabled: self.global_jvm_flags_enabled,
            jvm_flags: interface_config.global_jvm_flags.clone(),
            jvm_binary_enabled: self.global_jvm_binary_enabled,
            jvm_binary: interface_config.global_jvm_binary.clone(),
        });
    }
}

impl Render for SyncingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading == EnumSet::all() {
            let content = v_flex()
                .size_full()
                .p_3()
                .gap_3()
                .child("These options allow for syncing various files/folders across instances")
                .child(Spinner::new().with_size(gpui_component::Size::Large));
            return ui::page(cx, h_flex().gap_8().child("Syncing")).child(content).overflow_y_scrollbar();
        }

        let sync_folder = self.sync_state.sync_folder.clone();

        let warning = cx.theme().red;
        let info = cx.theme().blue;
        let content = v_flex()
            .size_full()
            .p_3()
            .gap_3()
            .child("These options allow for syncing various files/folders across instances")
            .when_some(sync_folder, |this, sync_folder| {
                this.child(
                    Button::new("open")
                        .info()
                        .icon(IconName::FolderOpen)
                        .label("Open synced folders directory")
                        .on_click(move |_, window, cx| {
                            crate::open_folder(&sync_folder, window, cx);
                        })
                        .w_72(),
                )
            })
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .child("Files"),
            )
            .child(self.create_entry("options", "Sync options.txt", SyncTarget::Options, warning, info, cx))
            .child(self.create_entry("servers", "Sync servers.dat", SyncTarget::Servers, warning, info, cx))
            .child(self.create_entry("commands", "Sync command_history.txt", SyncTarget::Commands, warning, info, cx))
            .child(self.create_entry("hotbars", "Sync hotbar.nbt", SyncTarget::Hotbars, warning, info, cx))
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .child("Folders"),
            )
            .child(self.create_entry("saves", "Sync saves folder", SyncTarget::Saves, warning, info, cx))
            .child(self.create_entry("config", "Sync config folder", SyncTarget::Config, warning, info, cx))
            .child(self.create_entry("screenshots", "Sync screenshots folder", SyncTarget::Screenshots, warning, info, cx))
            .child(self.create_entry("resourcepacks", "Sync resourcepacks folder", SyncTarget::Resourcepacks, warning, info, cx))
            .child(self.create_entry("shaderpacks", "Sync shaderpacks folder", SyncTarget::Shaderpacks, warning, info, cx))
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .child("Mods"),
            )
            .child(
                self.create_entry(
                    "flashback",
                    "Sync Flashback (flashback) folder",
                    SyncTarget::Flashback,
                    warning,
                    info,
                    cx,
                ),
            )
            .child(
                self.create_entry(
                    "dh",
                    "Sync Distant Horizons (Distant_Horizons_server_data) folder",
                    SyncTarget::DistantHorizons,
                    warning,
                    info,
                    cx,
                ),
            )
            .child(
                self.create_entry(
                    "voxy",
                    "Sync Voxy (.voxy) folder",
                    SyncTarget::Voxy,
                    warning,
                    info,
                    cx,
                ),
            )
            .child(
                self.create_entry(
                    "xaero",
                    "Sync Xaero's Minimap (xaero) folder",
                    SyncTarget::XaerosMinimap,
                    warning,
                    info,
                    cx,
                ),
            )
            .child(
                self.create_entry(
                    "bobby",
                    "Sync Bobby (.bobby) folder",
                    SyncTarget::Bobby,
                    warning,
                    info,
                    cx,
                ),
            )
            .child(
                self.create_entry(
                    "litematic",
                    "Litematic (schematic) folder",
                    SyncTarget::Litematic,
                    warning,
                    info,
                    cx,
                ),
            )
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .child("Global JVM Config"),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Checkbox::new("global_memory")
                                .label("Enable Global Memory")
                                .checked(self.global_memory_enabled)
                                .on_click(cx.listener(|page, value, _, cx| {
                                    page.global_memory_enabled = *value;
                                    InterfaceConfig::get_mut(cx).global_memory = if *value {
                                        Some(InstanceMemoryConfiguration {
                                            enabled: true,
                                            min: page.global_memory_min_input_state.read(cx).value().parse().unwrap_or(1024),
                                            max: page.global_memory_max_input_state.read(cx).value().parse().unwrap_or(2048),
                                        })
                                    } else {
                                        None
                                    };
                                    page.send_global_overrides(cx);
                                    cx.notify();
                                })))
                            .child("Memory (MiB)")
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(NumberInput::new(&self.global_memory_min_input_state).small().suffix("MiB").disabled(!self.global_memory_enabled))
                                    .child("Min")
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(NumberInput::new(&self.global_memory_max_input_state).small().suffix("MiB").disabled(!self.global_memory_enabled))
                                    .child("Max")
                            )
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Checkbox::new("global_jvm_flags")
                                .label("Enable Global JVM Flags")
                                .checked(self.global_jvm_flags_enabled)
                                .on_click(cx.listener(|page, value, _, cx| {
                                    page.global_jvm_flags_enabled = *value;
                                    let flags = page.global_jvm_flags_input_state.read(cx).value();
                                    InterfaceConfig::get_mut(cx).global_jvm_flags = if *value && !flags.is_empty() {
                                        Some(InstanceJvmFlagsConfiguration { enabled: true, flags: flags.into() })
                                    } else {
                                        None
                                    };
                                    page.send_global_overrides(cx);
                                    cx.notify();
                                })))
                            .child("JVM Flags")
                            .child(Input::new(&self.global_jvm_flags_input_state).disabled(!self.global_jvm_flags_enabled))
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Checkbox::new("global_jvm_binary")
                                .label("Enable Global JVM Binary")
                                .checked(self.global_jvm_binary_enabled)
                                .on_click(cx.listener(|page, value, _, cx| {
                                    page.global_jvm_binary_enabled = *value;
                                    InterfaceConfig::get_mut(cx).global_jvm_binary = if *value && page.global_jvm_binary_path.is_some() {
                                        page.global_jvm_binary_path.as_ref().map(|p| InstanceJvmBinaryConfiguration { enabled: true, path: Some(p.clone()) })
                                    } else {
                                        None
                                    };
                                    page.send_global_overrides(cx);
                                    cx.notify();
                                })))
                            .child("JVM Binary")
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("select_jvm_binary")
                                            .success()
                                            .label(
                                                self.global_jvm_binary_path
                                                    .as_ref()
                                                    .and_then(|p| p.file_name())
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or("Select JVM Binary")
                                                    .to_string()
                                            )
                                            .disabled(!self.global_jvm_binary_enabled)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let receiver = cx.prompt_for_paths(PathPromptOptions {
                                                    files: true,
                                                    directories: false,
                                                    multiple: false,
                                                    prompt: Some("Select JVM binary".into())
                                                });

                                                let this_entity = cx.entity();
                                                let add_from_file_task = window.spawn(cx, async move |cx| {
                                                    let Ok(result) = receiver.await else {
                                                        return;
                                                    };
                                                    _ = cx.update_window_entity(&this_entity, move |this, _window, cx| {
                                                        match result {
                                                            Ok(Some(paths)) => {
                                                                if let Some(path) = paths.first() {
                                                                    this.global_jvm_binary_path = Some(path.as_path().into());
                                                                    let jvm_binary = InstanceJvmBinaryConfiguration {
                                                                        enabled: true,
                                                                        path: Some(path.as_path().into()),
                                                                    };
                                                                    InterfaceConfig::get_mut(cx).global_jvm_binary = Some(jvm_binary.clone());
                                                                    this.send_global_overrides(cx);
                                                                }
                                                            },
                                                            _ => {},
                                                        }
                                                    });
                                                });

                                                this._select_jvm_binary_task = add_from_file_task;
                                            }))
                                    )
                            )
                    )
            );

        ui::page(cx, h_flex().gap_8().child("Syncing")).child(content).overflow_y_scrollbar()
    }
}
