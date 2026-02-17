use std::{borrow::Cow, cmp::Ordering, path::Path, sync::Arc};

use bridge::{
    handle::BackendHandle, instance::InstanceID, message::MessageToBackend, meta::MetadataRequest
};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonGroup, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent}, notification::{Notification, NotificationType}, select::{SearchableVec, Select, SelectEvent, SelectState}, skeleton::Skeleton, spinner::Spinner, v_flex, ActiveTheme as _, Disableable, Selectable, Sizable, WindowExt
};
use once_cell::sync::Lazy;
use schema::{fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, instance::{InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration, InstanceLinuxWrapperConfiguration, InstanceMemoryConfiguration, InstanceSystemLibrariesConfiguration, LwjglLibraryPath}, loader::Loader, version_manifest::MinecraftVersionManifest};
use strum::IntoEnumIterator;

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
    loader_select_state: Entity<SelectState<Vec<&'static str>>>,
    loader_versions_state: TypelessFrontendMetadataResult,
    loader_version_select_state: Entity<SelectState<SearchableVec<&'static str>>>,
    memory_override_enabled: bool,
    memory_min_input_state: Entity<InputState>,
    memory_max_input_state: Entity<InputState>,
    jvm_flags_enabled: bool,
    jvm_flags_input_state: Entity<InputState>,
    jvm_binary_enabled: bool,
    jvm_binary_path: Option<Arc<Path>>,

    override_glfw_enabled: bool,
    override_glfw_path: Option<Arc<Path>>,
    override_openal_enabled: bool,
    override_openal_path: Option<Arc<Path>>,

    #[cfg(target_os = "linux")]
    use_mangohud: bool,
    #[cfg(target_os = "linux")]
    use_gamemode: bool,
    #[cfg(target_os = "linux")]
    use_discrete_gpu: bool,
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
        #[cfg(target_os = "linux")]
        let linux_wrapper = entry.configuration.linux_wrapper.unwrap_or_default();
        let system_libraries = entry.configuration.system_libraries.clone().unwrap_or_default();

        let glfw_path = system_libraries.glfw.get_or_auto(&*AUTO_LIBRARY_PATH_GLFW);
        let openal_path = system_libraries.openal.get_or_auto(&*AUTO_LIBRARY_PATH_OPENAL);

        let new_name_input_state = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(&new_name_input_state, Self::on_new_name_input).detach();

        let minecraft_versions = FrontendMetadata::request(&data.metadata, MetadataRequest::MinecraftVersionManifest, cx);

        let version_select_state = cx.new(|cx| SelectState::new(VersionList::default(), None, window, cx).searchable(true));
        cx.observe_in(&minecraft_versions, window, |page, versions, window, cx| {
            page.update_minecraft_versions(versions, window, cx);
        }).detach();
        cx.subscribe(&version_select_state, Self::on_minecraft_version_selected).detach();

        let loader_select_state = cx.new(|cx| {
            let loaders = Loader::iter()
                .filter(|l| *l != Loader::Unknown)
                .map(|l| l.name())
                .collect();
            let mut state = SelectState::new(loaders, None, window, cx);
            state.set_selected_value(&loader.name(), window, cx);
            state
        });
        cx.subscribe_in(&loader_select_state, window, Self::on_loader_selected).detach();

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
            loader_select_state,
            loader_version_select_state,
            memory_override_enabled: memory.enabled,
            memory_min_input_state,
            memory_max_input_state,
            jvm_flags_enabled: jvm_flags.enabled,
            jvm_flags_input_state,
            jvm_binary_enabled: jvm_binary.enabled,
            jvm_binary_path: jvm_binary.path.clone(),
            override_glfw_enabled: system_libraries.override_glfw,
            override_glfw_path: glfw_path,
            override_openal_enabled: system_libraries.override_openal,
            override_openal_path: openal_path,
            #[cfg(target_os = "linux")]
            use_mangohud: linux_wrapper.use_mangohud,
            #[cfg(target_os = "linux")]
            use_gamemode: linux_wrapper.use_gamemode,
            #[cfg(target_os = "linux")]
            use_discrete_gpu: linux_wrapper.use_discrete_gpu,
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

    pub fn on_loader_selected(
        &mut self,
        _state: &Entity<SelectState<Vec<&'static str>>>,
        event: &SelectEvent<Vec<&'static str>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(value) = event;
        let Some(value) = value else {
            return;
        };

        let loader = Loader::from_name(value);
        if loader == Loader::Unknown {
            return;
        }

        if self.loader != loader {
            self.loader = loader;
            self.backend_handle.send(MessageToBackend::SetInstanceLoader {
                id: self.instance_id,
                loader: self.loader,
            });
            self.update_loader_versions(window, cx);
            cx.notify();
        }
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

    fn get_system_libraries_configuration(&self) -> InstanceSystemLibrariesConfiguration {
        InstanceSystemLibrariesConfiguration {
            override_glfw: self.override_glfw_enabled,
            glfw: Self::create_lwjgl_library_path(&self.override_glfw_path, &*AUTO_LIBRARY_PATH_GLFW),
            override_openal: self.override_openal_enabled,
            openal: Self::create_lwjgl_library_path(&self.override_openal_path, &*AUTO_LIBRARY_PATH_OPENAL),

        }
    }

    fn create_lwjgl_library_path(path: &Option<Arc<Path>>, auto: &Option<Arc<Path>>) -> LwjglLibraryPath {
        if let Some(path) = path {
            if let Some(auto) = auto && path == auto {
                LwjglLibraryPath::Auto(path.clone())
            } else {
                LwjglLibraryPath::Explicit(path.clone())
            }
        } else {
            LwjglLibraryPath::None
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_wrapper_configuration(&self) -> InstanceLinuxWrapperConfiguration {
        InstanceLinuxWrapperConfiguration {
            use_mangohud: self.use_mangohud,
            use_gamemode: self.use_gamemode,
            use_discrete_gpu: self.use_discrete_gpu,
        }
    }

    #[cfg(target_os = "linux")]
    fn is_command_available(command: &str) -> bool {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {command}"))
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn select_file(&mut self, message: &'static str, handle: impl FnOnce(&mut Self, Option<Arc<Path>>) + 'static, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::new_static(message))
        });

        let this_entity = cx.entity();
        self._select_file_task = window.spawn(cx, async move |cx| {
            let Ok(result) = receiver.await else {
                return;
            };
            _ = cx.update_window_entity(&this_entity, move |this, window, cx| {
                match result {
                    Ok(Some(paths)) => {
                        (handle)(this, paths.first().map(|v| v.as_path().into()));
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

        let jvm_binary_label = opt_path_to_string(&self.jvm_binary_path);
        let glfw_path_label = opt_path_to_string(&self.override_glfw_path);
        let openal_path_label = opt_path_to_string(&self.override_openal_path);

        let mut basic_content = v_flex()
            .gap_4()
            .size_full()
            .child(crate::labelled(
                "Instance Name",
                h_flex()
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

        let mut version_content = v_flex().gap_2();

        match self.version_state {
            TypelessFrontendMetadataResult::Loading => {
                version_content = version_content.child(Skeleton::new().w_full().min_h_8().max_h_8().rounded_md());
            },
            TypelessFrontendMetadataResult::Loaded => {
                version_content = version_content.child(Select::new(&self.version_select_state).w_full());
            },
            TypelessFrontendMetadataResult::Error(ref error) => {
                version_content = version_content.child(format!("Error loading minecraft versions: {}", error))
            },
        }

        version_content = version_content.child(Select::new(&self.loader_select_state).title_prefix("Modloader: ").w_full());

        if self.loader != Loader::Vanilla {
            match self.loader_versions_state {
                TypelessFrontendMetadataResult::Loading => {
                    version_content = version_content.child(Skeleton::new().w_full().min_h_8().max_h_8().rounded_md())
                },
                TypelessFrontendMetadataResult::Loaded => {
                    version_content = version_content.child(Select::new(&self.loader_version_select_state).title_prefix(match self.loader {
                        Loader::Fabric => "Fabric Version: ",
                        Loader::Forge => "Forge Version: ",
                        Loader::NeoForge => "NeoForge Version: ",
                        Loader::Vanilla | Loader::Unknown => "Loader Version: ",
                    }).w_full())
                },
                TypelessFrontendMetadataResult::Error(ref error) => {
                    version_content = version_content.child(format!("Error loading possible loader versions: {}", error))
                },
            }
        }

        basic_content = basic_content.child(crate::labelled(
            "Version",
            version_content,
        ));

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
                    .child(v_flex()
                        .w_full()
                        .gap_1()
                        .child(NumberInput::new(&self.memory_min_input_state).small().suffix("MiB").disabled(!memory_override_enabled))
                        .child(NumberInput::new(&self.memory_max_input_state).small().suffix("MiB").disabled(!memory_override_enabled))
                    )
                    .child(v_flex()
                        .gap_1()
                        .child("Min")
                        .child("Max"))
                )
            ).child(v_flex()
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
                    this.select_file("Select Jvm Binary", |this, path| {
                        this.jvm_binary_path = path;
                        this.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                            id: this.instance_id,
                            jvm_binary: this.get_jvm_binary_configuration()
                        });
                    }, window, cx);
                })))
            )
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("system_glfw").label("Use System GLFW").checked(self.override_glfw_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.override_glfw_enabled != *value {
                        page.override_glfw_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: page.instance_id,
                            system_libraries: page.get_system_libraries_configuration()
                        });
                        cx.notify();

                    }
                })))
                .child(Button::new("select_glfw").success().label(glfw_path_label).disabled(!self.override_glfw_enabled).on_click(cx.listener(|this, _, window, cx| {
                    this.select_file("Select GLFW Library", |this, path| {
                        this.override_glfw_path = path;
                        this.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: this.instance_id,
                            system_libraries: this.get_system_libraries_configuration()
                        });
                    }, window, cx);
                })))
            )
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("system_openal").label("Use System OpenAL").checked(self.override_openal_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.override_openal_enabled != *value {
                        page.override_openal_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: page.instance_id,
                            system_libraries: page.get_system_libraries_configuration()
                        });
                        cx.notify();

                    }
                })))
                .child(Button::new("select_openal").success().label(openal_path_label).disabled(!self.override_openal_enabled).on_click(cx.listener(|this, _, window, cx| {
                    this.select_file("Select OpenAL Library", |this, path| {
                        this.override_openal_path = path;
                        this.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: this.instance_id,
                            system_libraries: this.get_system_libraries_configuration()
                        });
                    }, window, cx);
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
            .child(Checkbox::new("use_discrete_gpu").label("Use Discrete GPU").checked(self.use_discrete_gpu).on_click(cx.listener(|page, value, _, cx| {
                if page.use_discrete_gpu != *value {
                    page.use_discrete_gpu = *value;
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
            .items_start()
            .p_4()
            .gap_4()
            .child(basic_content)
            .child(div().bg(cx.theme().border).h_full().min_w_px().max_w_px().w_px())
            .child(runtime_content)
            .child(div().bg(cx.theme().border).h_full().min_w_px().max_w_px().w_px())
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

fn opt_path_to_string(path: &Option<Arc<Path>>) -> SharedString {
    if let Some(path) = path {
        SharedString::new(path.to_string_lossy())
    } else {
        SharedString::new_static("<unset>")
    }
}

static AUTO_LIBRARY_PATH_GLFW: Lazy<Option<Arc<Path>>> = Lazy::new(|| get_shared_library_path_for_name("glfw"));
static AUTO_LIBRARY_PATH_OPENAL: Lazy<Option<Arc<Path>>> = Lazy::new(|| get_shared_library_path_for_name("openal"));

#[cfg(not(unix))]
fn get_shared_library_path_for_name(name: &str) -> Option<Arc<Path>> {
    None
}

#[cfg(unix)]
fn get_shared_library_path_for_name(name: &str) -> Option<Arc<Path>> {
    let filename = format!("{}{}{}", std::env::consts::DLL_PREFIX, name, std::env::consts::DLL_SUFFIX);

    let search_paths = &[
        "/lib/",
        "/lib64/",
        "/usr/lib/",
        "/usr/lib64/",
        "/usr/local/lib/",
        #[cfg(target_os = "macos")]
        "/opt/homebrew/lib/"
    ];

    for search_path in search_paths {
        let path = Path::new(search_path).join(&filename);
        if path.exists() {
            return Some(path.into());
        }
    }

    None
}
