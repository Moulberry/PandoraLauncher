use std::{borrow::Cow, cmp::Ordering, path::Path, sync::Arc};
#[cfg(target_os = "linux")]
use std::process::Command;

use bridge::{
    handle::BackendHandle, instance::InstanceID, message::MessageToBackend, meta::MetadataRequest
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Disableable, Selectable, Sizable, WindowExt, button::{Button, ButtonGroup, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent}, notification::{Notification, NotificationType}, select::{SearchableVec, Select, SelectEvent, SelectState}, spinner::Spinner, v_flex
};
use schema::{fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, instance::{InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration, InstanceLinuxWrapperConfiguration, InstanceMemoryConfiguration}, loader::Loader, version_manifest::MinecraftVersionManifest};

use crate::{entity::{DataEntities, instance::InstanceEntry, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState, TypelessFrontendMetadataResult}}, interface_config::InterfaceConfig, pages::instances_page::VersionList};

#[derive(PartialEq, Eq)]
enum NewNameChangeState {
    NoChange,
    InvalidName,
    Pending,
}

pub struct InstanceSettingsSubpage {
    data: DataEntities,
    instance: Entity<InstanceEntry>,
    instance_id: InstanceID,
    new_name_input_state: Entity<InputState>,
    version_state: TypelessFrontendMetadataResult,
    version_select_state: Entity<SelectState<VersionList>>,
    loader: Loader,
    loader_versions_state: TypelessFrontendMetadataResult,
    loader_version_select_state: Entity<SelectState<SearchableVec<&'static str>>>,
    memory_override_enabled: bool,
    memory_min_input_state: Entity<InputState>,
    memory_max_input_state: Entity<InputState>,
    jvm_flags_enabled: bool,
    jvm_flags_input_state: Entity<InputState>,
    jvm_binary_enabled: bool,
    jvm_binary_path: Option<Arc<Path>>,
    #[cfg(target_os = "linux")]
    use_mangohud: bool,
    #[cfg(target_os = "linux")]
    use_gamemode: bool,
    #[cfg(target_os = "linux")]
    mangohud_available: bool,
    #[cfg(target_os = "linux")]
    gamemode_available: bool,
    new_name_change_state: NewNameChangeState,
    backend_handle: BackendHandle,
    _observe_loader_version_subscription: Option<Subscription>,
    _select_file_task: Task<()>,
}

impl InstanceSettingsSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        data: &DataEntities,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let entry = instance.read(cx);
        let instance_id = entry.id;
        let loader = entry.configuration.loader;
        let preferred_loader_version = entry.configuration.preferred_loader_version.map(|s| s.as_str()).unwrap_or("Latest");

        let memory = entry.configuration.memory.unwrap_or_default();
        let jvm_flags = entry.configuration.jvm_flags.clone().unwrap_or_default();
        let jvm_binary = entry.configuration.jvm_binary.clone().unwrap_or_default();
        let linux_wrapper = entry.configuration.linux_wrapper.unwrap_or_default();

        let new_name_input_state = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(&new_name_input_state, Self::on_new_name_input).detach();

        let minecraft_versions = FrontendMetadata::request(&data.metadata, MetadataRequest::MinecraftVersionManifest, cx);

        let version_select_state = cx.new(|cx| SelectState::new(VersionList::default(), None, window, cx).searchable(true));
        cx.observe_in(&minecraft_versions, window, |page, versions, window, cx| {
            page.update_minecraft_versions(versions, window, cx);
        }).detach();
        cx.subscribe(&version_select_state, Self::on_minecraft_version_selected).detach();

        cx.observe_in(instance, window, |page, instance, window, cx| {
            if page.loader_version_select_state.read(cx).selected_index(cx).is_none() {
                let version = instance.read(cx).configuration.preferred_loader_version.map(|s| s.as_str()).unwrap_or("Latest");
                page.loader_version_select_state.update(cx, |select_state, cx| {
                    select_state.set_selected_value(&version, window, cx);
                });
            }
        }).detach();

        let loader_version_select_state = cx.new(|cx| {
            let mut select_state = SelectState::new(SearchableVec::new(vec![]), None, window, cx).searchable(true);
            select_state.set_selected_value(&preferred_loader_version, window, cx);
            select_state
        });
        cx.subscribe(&loader_version_select_state, Self::on_loader_version_selected).detach();

        let memory_min_input_state = cx.new(|cx| {
            InputState::new(window, cx).default_value(memory.min.to_string())
        });
        cx.subscribe_in(&memory_min_input_state, window, Self::on_memory_step).detach();
        cx.subscribe(&memory_min_input_state, Self::on_memory_changed).detach();
        let memory_max_input_state = cx.new(|cx| {
            InputState::new(window, cx).default_value(memory.max.to_string())
        });
        cx.subscribe_in(&memory_max_input_state, window, Self::on_memory_step).detach();
        cx.subscribe(&memory_max_input_state, Self::on_memory_changed).detach();

        let jvm_flags_input_state = cx.new(|cx| {
            InputState::new(window, cx).auto_grow(1, 8).default_value(jvm_flags.flags)
        });
        cx.subscribe(&jvm_flags_input_state, Self::on_jvm_flags_changed).detach();

        let mut page = Self {
            data: data.clone(),
            instance: instance.clone(),
            instance_id,
            new_name_input_state,
            version_state: TypelessFrontendMetadataResult::Loading,
            version_select_state,
            loader,
            loader_version_select_state,
            memory_override_enabled: memory.enabled,
            memory_min_input_state,
            memory_max_input_state,
            jvm_flags_enabled: jvm_flags.enabled,
            jvm_flags_input_state,
            jvm_binary_enabled: jvm_binary.enabled,
            jvm_binary_path: jvm_binary.path.clone(),
            #[cfg(target_os = "linux")]
            use_mangohud: linux_wrapper.use_mangohud,
            #[cfg(target_os = "linux")]
            use_gamemode: linux_wrapper.use_gamemode,
            #[cfg(target_os = "linux")]
            mangohud_available: Self::is_command_available("mangohud"),
            #[cfg(target_os = "linux")]
            gamemode_available: Self::is_command_available("gamemoderun"),
            new_name_change_state: NewNameChangeState::NoChange,
            backend_handle,
            loader_versions_state: TypelessFrontendMetadataResult::Loading,
            _observe_loader_version_subscription: None,
            _select_file_task: Task::ready(())
        };
        page.update_minecraft_versions(minecraft_versions, window, cx);
        page.update_loader_versions(window, cx);
        page
    }
}

impl InstanceSettingsSubpage {
    fn update_minecraft_versions(&mut self, versions: Entity<FrontendMetadataState>, window: &mut Window, cx: &mut Context<Self>) {
        let result: FrontendMetadataResult<MinecraftVersionManifest> = versions.read(cx).result();
        let versions = match result {
            FrontendMetadataResult::Loading => {
                Vec::new()
            },
            FrontendMetadataResult::Error(_) => {
                Vec::new()
            },
            FrontendMetadataResult::Loaded(manifest) => {
                manifest.versions.iter().map(|v| SharedString::from(v.id.as_str())).collect()
            },
        };

        let current_version = self.instance.read(cx).configuration.minecraft_version;

        self.version_state = result.as_typeless();

        self.version_select_state.update(cx, |dropdown, cx| {
            let mut to_select = None;

            if let Some(last_selected) = dropdown.selected_value().cloned()
                && versions.contains(&last_selected)
            {
                to_select = Some(last_selected);
            }

            if to_select.is_none()
                && versions.contains(&SharedString::new_static(current_version.as_str()))
            {
                to_select = Some(SharedString::new_static(current_version.as_str()));
            }

            dropdown.set_items(
                VersionList {
                    versions: versions.clone(),
                    matched_versions: versions,
                },
                window,
                cx,
            );

            if let Some(to_select) = to_select {
                dropdown.set_selected_value(&to_select, window, cx);
            }

            cx.notify();
        });
    }

    fn update_loader_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let loader_versions = match self.loader {
            Loader::Vanilla | Loader::Unknown => {
                self._observe_loader_version_subscription = None;
                self.loader_versions_state = TypelessFrontendMetadataResult::Loaded;
                vec![""]
            },
            Loader::Fabric => {
                self.update_loader_versions_for_loader(MetadataRequest::FabricLoaderManifest, |manifest: &FabricLoaderManifest| {
                    std::iter::once("Latest")
                        .chain(manifest.0.iter().map(|s| s.version.as_str()))
                        .collect()
                }, window, cx)
            },
            Loader::Forge => {
                self.update_loader_versions_for_loader(MetadataRequest::ForgeMavenManifest, |manifest: &ForgeMavenManifest| {
                    std::iter::once("Latest")
                        .chain(manifest.0.iter().map(|s| s.as_str()))
                        .collect()
                }, window, cx)
            },
            Loader::NeoForge => {
                self.update_loader_versions_for_loader(MetadataRequest::NeoforgeMavenManifest, |manifest: &NeoforgeMavenManifest| {
                    std::iter::once("Latest")
                        .chain(manifest.0.iter().map(|s| s.as_str()))
                        .collect()
                }, window, cx)
            },
        };
        let preferred_loader_version = self.instance.read(cx).configuration.preferred_loader_version.map(|s| s.as_str()).unwrap_or("Latest");
        self.loader_version_select_state.update(cx, move |select_state, cx| {
            select_state.set_items(SearchableVec::new(loader_versions), window, cx);
            select_state.set_selected_value(&preferred_loader_version, window, cx);
        });
    }

    fn update_loader_versions_for_loader<T>(
        &mut self,
        request: MetadataRequest,
        items_fn: impl Fn(&T) -> Vec<&'static str> + 'static,
        window: &mut Window,
        cx: &mut Context<Self>
    ) -> Vec<&'static str>
    where
        FrontendMetadataState: AsMetadataResult<T>,
    {
        let request = FrontendMetadata::request(&self.data.metadata, request, cx);

        let result: FrontendMetadataResult<T> = request.read(cx).result();
        let items = match &result {
            FrontendMetadataResult::Loading => vec![],
            FrontendMetadataResult::Loaded(manifest) => (items_fn)(&manifest),
            FrontendMetadataResult::Error(_) => vec![],
        };
        self.loader_versions_state = result.as_typeless();
        self._observe_loader_version_subscription = Some(cx.observe_in(&request, window, move |page, metadata, window, cx| {
            let result: FrontendMetadataResult<T> = metadata.read(cx).result();
            let versions = if let FrontendMetadataResult::Loaded(manifest) = &result {
                (items_fn)(&manifest)
            } else {
                vec![]
            };
            page.loader_versions_state = result.as_typeless();
            let preferred_loader_version = page.instance.read(cx).configuration.preferred_loader_version.map(|s| s.as_str()).unwrap_or("Latest");
            page.loader_version_select_state.update(cx, move |select_state, cx| {
                select_state.set_items(SearchableVec::new(versions), window, cx);
                select_state.set_selected_value(&preferred_loader_version, window, cx);
            });
        }));
        items
    }

    pub fn on_new_name_input(
        &mut self,
        state: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            let new_name = state.read(cx).value();
            if new_name.is_empty() {
                self.new_name_change_state = NewNameChangeState::NoChange;
                return;
            }

            let instance = self.instance.read(cx);
            if instance.name == new_name {
                self.new_name_change_state = NewNameChangeState::NoChange;
                return;
            }

            if !crate::is_valid_instance_name(new_name.as_str()) {
                self.new_name_change_state = NewNameChangeState::InvalidName;
                return;
            }

            self.new_name_change_state = NewNameChangeState::Pending;
        }
    }

    pub fn on_minecraft_version_selected(
        &mut self,
        _state: Entity<SelectState<VersionList>>,
        event: &SelectEvent<VersionList>,
        _cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(value) = event;

        let Some(value) = value else {
            return;
        };

        self.backend_handle.send(MessageToBackend::SetInstanceMinecraftVersion {
            id: self.instance_id,
            version: value.as_str().into(),
        });
    }

    pub fn on_loader_version_selected(
        &mut self,
        _state: Entity<SelectState<SearchableVec<&'static str>>>,
        event: &SelectEvent<SearchableVec<&'static str>>,
        _cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(value) = event;

        let value = if value == &Some("Latest") {
            None
        } else {
            value.clone()
        };

        self.backend_handle.send(MessageToBackend::SetInstancePreferredLoaderVersion {
            id: self.instance_id,
            loader_version: value,
        });
    }

    pub fn on_memory_step(
        &mut self,
        state: &Entity<InputState>,
        event: &NumberInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            NumberInputEvent::Step(step_action) => match step_action {
                gpui_component::input::StepAction::Decrement => {
                    if let Ok(mut value) = state.read(cx).value().parse::<u32>() {
                        value = value.saturating_div(256).saturating_sub(1).saturating_mul(256).max(128);
                        state.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx);
                        })
                    }
                },
                gpui_component::input::StepAction::Increment => {
                    if let Ok(mut value) = state.read(cx).value().parse::<u32>() {
                        value = value.saturating_div(256).saturating_add(1).saturating_mul(256).max(128);
                        state.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx);
                        })
                    }
                },
            },
        }
    }

    pub fn on_memory_changed(
        &mut self,
        _: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            self.backend_handle.send(MessageToBackend::SetInstanceMemory {
                id: self.instance_id,
                memory: self.get_memory_configuration(cx)
            });
        }
    }

    fn get_memory_configuration(&self, cx: &App) -> InstanceMemoryConfiguration {
        let min = self.memory_min_input_state.read(cx).value().parse::<u32>().unwrap_or(0);
        let max = self.memory_max_input_state.read(cx).value().parse::<u32>().unwrap_or(0);

        InstanceMemoryConfiguration {
            enabled: self.memory_override_enabled,
            min,
            max
        }
    }

    pub fn on_jvm_flags_changed(
        &mut self,
        _: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            self.backend_handle.send(MessageToBackend::SetInstanceJvmFlags {
                id: self.instance_id,
                jvm_flags: self.get_jvm_flags_configuration(cx)
            });
        }
    }

    fn get_jvm_flags_configuration(&self, cx: &App) -> InstanceJvmFlagsConfiguration {
        let flags = self.jvm_flags_input_state.read(cx).value();

        InstanceJvmFlagsConfiguration {
            enabled: self.jvm_flags_enabled,
            flags: flags.into(),
        }
    }

    fn get_jvm_binary_configuration(&self) -> InstanceJvmBinaryConfiguration {
        InstanceJvmBinaryConfiguration {
            enabled: self.jvm_binary_enabled,
            path: self.jvm_binary_path.clone(),
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_wrapper_configuration(&self) -> InstanceLinuxWrapperConfiguration {
        InstanceLinuxWrapperConfiguration {
            use_mangohud: self.use_mangohud,
            use_gamemode: self.use_gamemode,
        }
    }

    #[cfg(target_os = "linux")]
    fn is_command_available(command: &str) -> bool {
        Command::new("which")
            .arg(command)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl Render for InstanceSettingsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child("Settings"));

        let memory_override_enabled = self.memory_override_enabled;
        let jvm_flags_enabled = self.jvm_flags_enabled;
        let jvm_binary_enabled = self.jvm_binary_enabled;

        let jvm_binary_label = if let Some(path) = &self.jvm_binary_path {
            SharedString::new(path.to_string_lossy())
        } else {
            SharedString::new_static("<unset>")
        };

        let mut basic_content = v_flex()
            .gap_4()
            .size_full()
            .child(v_flex()
                .child("Instance name")
                .child(h_flex()
                    .gap_2()
                    .child(Input::new(&self.new_name_input_state))
                    .when(self.new_name_change_state != NewNameChangeState::NoChange, |this| {
                        if self.new_name_change_state == NewNameChangeState::InvalidName {
                            this.child("Invalid name")
                        } else {
                            this.child(Button::new("setname").label("Update").on_click({
                                let instance = self.instance.clone();
                                let backend_handle = self.backend_handle.clone();
                                let new_name = self.new_name_input_state.read(cx).value();
                                move |_, _, cx| {
                                    let instance = instance.read(cx);
                                    let id = instance.id;
                                    backend_handle.send(MessageToBackend::RenameInstance {
                                        id,
                                        name: new_name.as_str().into(),
                                    });
                                }
                            }))
                        }
                    })
                )
            );

        match self.version_state {
            TypelessFrontendMetadataResult::Loading => {
                basic_content = basic_content.child(crate::labelled(
                    "Version",
                    Spinner::new()
                ))
            },
            TypelessFrontendMetadataResult::Loaded => {
                basic_content = basic_content.child(crate::labelled(
                    "Version",
                    Select::new(&self.version_select_state).w_full()
                ))
            },
            TypelessFrontendMetadataResult::Error(ref error) => {
                basic_content = basic_content.child(format!("Error loading minecraft versions: {}", error))
            },
        }

        basic_content = basic_content
            .child(ButtonGroup::new("loader")
                .outline()
                .child(
                    Button::new("loader-vanilla")
                        .label("Vanilla")
                        .selected(self.loader == Loader::Vanilla),
                )
                .child(
                    Button::new("loader-fabric")
                        .label("Fabric")
                        .selected(self.loader == Loader::Fabric),
                )
                .child(
                    Button::new("loader-forge")
                        .label("Forge")
                        .selected(self.loader == Loader::Forge),
                )
                .child(
                    Button::new("loader-neoforge")
                        .label("NeoForge")
                        .selected(self.loader == Loader::NeoForge),
                )
                .on_click(cx.listener({
                    let backend_handle = self.backend_handle.clone();
                    move |page, selected: &Vec<usize>, window, cx| {
                        let last_loader = page.loader;
                        match selected.first() {
                            Some(0) => page.loader = Loader::Vanilla,
                            Some(1) => page.loader = Loader::Fabric,
                            Some(2) => page.loader = Loader::Forge,
                            Some(3) => page.loader = Loader::NeoForge,
                            _ => {},
                        };
                        if page.loader != last_loader {
                            backend_handle.send(MessageToBackend::SetInstanceLoader {
                                id: page.instance_id,
                                loader: page.loader,
                            });
                            page.update_loader_versions(window, cx);
                            cx.notify();
                        }
                    }
                }))
            );

        if self.loader != Loader::Vanilla {
            match self.loader_versions_state {
                TypelessFrontendMetadataResult::Loading => {
                    basic_content = basic_content.child(crate::labelled(
                        "Loader Version",
                        Spinner::new()
                    ))
                },
                TypelessFrontendMetadataResult::Loaded => {
                    basic_content = basic_content.child(crate::labelled(
                        "Loader Version",
                        Select::new(&self.loader_version_select_state).w_full()
                    ))
                },
                TypelessFrontendMetadataResult::Error(ref error) => {
                    basic_content = basic_content.child(format!("Error loading possible loader versions: {}", error))
                },
            }
        }

        let runtime_content = v_flex()
            .gap_4()
            .size_full()
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("memory").label("Set Memory").checked(memory_override_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.memory_override_enabled != *value {
                        page.memory_override_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceMemory {
                            id: page.instance_id,
                            memory: page.get_memory_configuration(cx)
                        });
                        cx.notify();
                    }
                })))
                .child(h_flex()
                    .gap_1()
                    .child(NumberInput::new(&self.memory_min_input_state).small().suffix("MiB").disabled(!memory_override_enabled))
                    .child("Min"))
                .child(h_flex()
                    .gap_1()
                    .child(NumberInput::new(&self.memory_max_input_state).small().suffix("MiB").disabled(!memory_override_enabled))
                    .child("Max"))
                )
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("jvm_flags").label("Add JVM Flags").checked(jvm_flags_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.jvm_flags_enabled != *value {
                        page.jvm_flags_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceJvmFlags {
                            id: page.instance_id,
                            jvm_flags: page.get_jvm_flags_configuration(cx)
                        });
                        cx.notify();
                    }
                })))
                .child(Input::new(&self.jvm_flags_input_state).disabled(!jvm_flags_enabled))
            )
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("jvm_binary").label("Override JVM Binary").checked(jvm_binary_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.jvm_binary_enabled != *value {
                        page.jvm_binary_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                            id: page.instance_id,
                            jvm_binary: page.get_jvm_binary_configuration()
                        });
                        cx.notify();
                    }
                })))
                .child(Button::new("select_jvm_binary").success().label(jvm_binary_label).disabled(!jvm_binary_enabled).on_click(cx.listener(|this, _, window, cx| {
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
                        _ = cx.update_window_entity(&this_entity, move |this, window, cx| {
                            match result {
                                Ok(Some(paths)) => {
                                    this.jvm_binary_path = paths.first().map(|v| v.as_path().into());
                                    this.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                                        id: this.instance_id,
                                        jvm_binary: this.get_jvm_binary_configuration()
                                    });
                                    cx.notify();
                                },
                                Ok(None) => {},
                                Err(error) => {
                                    let error = format!("{}", error);
                                    let notification = Notification::new()
                                        .autohide(false)
                                        .with_type(NotificationType::Error)
                                        .title(error);
                                    window.push_notification(notification, cx);
                                },
                            }
                        });
                    });
                    this._select_file_task = add_from_file_task;
                })))
            );

        #[cfg(target_os = "linux")]
        let runtime_content = runtime_content.child(v_flex()
            .gap_1()
            .child("Linux Performance")
            .child(Checkbox::new("use_mangohud").label("Use MangoHud").checked(self.use_mangohud).disabled(!self.mangohud_available).on_click(cx.listener(|page, value, _, cx| {
                if page.use_mangohud != *value {
                    page.use_mangohud = *value;
                    page.backend_handle.send(MessageToBackend::SetInstanceLinuxWrapper {
                        id: page.instance_id,
                        linux_wrapper: page.get_linux_wrapper_configuration()
                    });
                    cx.notify();
                }
            })))
            .child(Checkbox::new("use_gamemode").label("Use GameMode").checked(self.use_gamemode).disabled(!self.gamemode_available).on_click(cx.listener(|page, value, _, cx| {
                if page.use_gamemode != *value {
                    page.use_gamemode = *value;
                    page.backend_handle.send(MessageToBackend::SetInstanceLinuxWrapper {
                        id: page.instance_id,
                        linux_wrapper: page.get_linux_wrapper_configuration()
                    });
                    cx.notify();
                }
            })))
        );

        let actions_content = v_flex()
            .gap_4()
            .size_full()
            .child(Button::new("shortcut").label("Create shortcut").success().on_click({
                let instance = self.instance.clone();
                let backend_handle = self.backend_handle.clone();
                move |_: &ClickEvent, _, cx| {
                    let user_dirs = directories::UserDirs::new();
                    let directory = user_dirs.as_ref()
                        .and_then(directories::UserDirs::desktop_dir).unwrap_or(Path::new("."));
                    let instance = instance.read(cx);
                    let id = instance.id;
                    let name = instance.name.clone();

                    #[cfg(target_os = "linux")]
                    let suggested_name = format!("{name}.desktop");
                    #[cfg(target_os = "windows")]
                    let suggested_name = format!("{name}.lnk");
                    #[cfg(target_os = "macos")]
                    let suggested_name = format!("{name}.app");

                    let receiver = cx.prompt_for_new_path(directory, Some(&suggested_name));
                    let backend_handle = backend_handle.clone();
                    cx.spawn(async move |_| {
                        let Ok(Ok(Some(path))) = receiver.await else {
                            return;
                        };
                        backend_handle.send(MessageToBackend::CreateInstanceShortcut { id, path });
                    }).detach();
                }
            }))
            .child(Button::new("delete").label("Delete this instance").danger().on_click({
                let instance = self.instance.clone();
                let backend_handle = self.backend_handle.clone();
                move |click: &ClickEvent, window, cx| {
                    let instance = instance.read(cx);
                    let id = instance.id;
                    let name = instance.name.clone();

                    if InterfaceConfig::get(cx).quick_delete_instance && click.modifiers().shift {
                        backend_handle.send(bridge::message::MessageToBackend::DeleteInstance {
                            id
                        });
                    } else {
                        crate::modals::delete_instance::open_delete_instance(id, name, backend_handle.clone(), window, cx);
                    }

                }
            }));

        let sections = h_flex()
            .size_full()
            .justify_evenly()
            .p_4()
            .gap_4()
            .child(basic_content)
            .child(div().bg(cx.theme().border).h_full().w_0p5())
            .child(runtime_content)
            .child(div().bg(cx.theme().border).h_full().w_0p5())
            .child(actions_content);

        v_flex()
            .p_4()
            .size_full()
            .child(header)
            .child(div()
                .size_full()
                .border_1()
                .rounded(theme.radius)
                .border_color(theme.border)
                .child(sections)
            )
    }
}
