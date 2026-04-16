use std::{path::Path, sync::Arc};

use bridge::{
    handle::BackendHandle, instance::InstanceID, message::{EmbeddedOrRaw, MessageToBackend}, meta::MetadataRequest
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IndexPath, Sizable, WindowExt, button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent}, notification::{Notification, NotificationType}, select::{SearchableVec, Select, SelectEvent, SelectState}, skeleton::Skeleton, v_flex
};
use schema::{fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, instance::{AUTO_LIBRARY_PATH_GLFW, AUTO_LIBRARY_PATH_OPENAL, InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration, InstanceLinuxWrapperConfiguration, InstanceMemoryConfiguration, InstanceSystemLibrariesConfiguration, InstanceWrapperCommandConfiguration, LwjglLibraryPath}, loader::Loader, version_manifest::MinecraftVersionManifest};
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
	component::{horizontal_sections::HorizontalSections, named_dropdown::{NamedDropdown, NamedDropdownItem}, path_label::PathLabel},
	entity::{DataEntities, account::{AccountEntries, AccountExt}, instance::InstanceEntry, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState, TypelessFrontendMetadataResult}},
	interface_config::InterfaceConfig, pages::instances_page::VersionList, png_render_cache,
};

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
    account_items: Entity<SelectState<NamedDropdown<Uuid>>>,
    loader: Loader,
    loader_select_state: Entity<SelectState<Vec<&'static str>>>,
    loader_versions_state: TypelessFrontendMetadataResult,
    loader_version_select_state: Entity<SelectState<SearchableVec<&'static str>>>,
    disable_file_syncing: bool,
    sandbox_available: bool,
    sandbox: bool,

    memory_override_enabled: bool,
    memory_min_input_state: Entity<InputState>,
    memory_max_input_state: Entity<InputState>,
    wrapper_command_enabled: bool,
    wrapper_command_input_state: Entity<InputState>,
    jvm_flags_enabled: bool,
    jvm_flags_input_state: Entity<InputState>,
    jvm_binary_enabled: bool,
    jvm_binary_path: Option<PathLabel>,

    instance_root_label: PathLabel,

    override_glfw_enabled: bool,
    override_glfw_path: Option<PathLabel>,
    override_openal_enabled: bool,
    override_openal_path: Option<PathLabel>,

    #[cfg(target_os = "linux")]
    use_mangohud: bool,
    #[cfg(target_os = "linux")]
    use_gamemode: bool,
    #[cfg(target_os = "linux")]
    use_discrete_gpu: bool,
    #[cfg(target_os = "linux")]
    disable_gl_threaded_optimizations: bool,
    #[cfg(target_os = "linux")]
    mangohud_available: bool,
    #[cfg(target_os = "linux")]
    gamemode_available: bool,
    new_name_change_state: NewNameChangeState,
    icon: Option<EmbeddedOrRaw>,
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
        let instance_name = entry.name.clone();
        let loader = entry.configuration.loader;
        let preferred_loader_version = entry.configuration.preferred_loader_version.map(|s| s.as_str()).unwrap_or("Latest");
        let account = entry.configuration.preferred_account;
        let disable_file_syncing = entry.configuration.disable_file_syncing;
        let sandbox = entry.configuration.sandbox;

        let sandbox_available = if cfg!(target_os = "linux") {
            command::is_command_available("bwrap") && command::is_command_available("xdg-dbus-proxy")
        } else {
            true
        };

        let memory = entry.configuration.memory.unwrap_or_default();
        let wrapper_command = entry.configuration.wrapper_command.clone().unwrap_or_default();
        let jvm_flags = entry.configuration.jvm_flags.clone().unwrap_or_default();
        let jvm_binary = entry.configuration.jvm_binary.clone().unwrap_or_default();
        #[cfg(target_os = "linux")]
        let linux_wrapper = entry.configuration.linux_wrapper.unwrap_or_default();
        let system_libraries = entry.configuration.system_libraries.clone().unwrap_or_default();

        let instance_root_label = PathLabel::new(entry.root_path.clone(), true);

        let icon = if let Some(raw) = entry.icon.clone() {
            Some(EmbeddedOrRaw::Raw(raw))
        } else if let Some(embedded) = entry.configuration.instance_fallback_icon {
            Some(EmbeddedOrRaw::Embedded(embedded.as_str().into()))
        } else {
            None
        };

        let glfw_path = system_libraries.glfw.get_or_auto(&*AUTO_LIBRARY_PATH_GLFW);
        let openal_path = system_libraries.openal.get_or_auto(&*AUTO_LIBRARY_PATH_OPENAL);

        let new_name_input_state = cx.new(|cx| {
            InputState::new(window, cx).default_value(instance_name)
        });
        cx.subscribe(&new_name_input_state, Self::on_new_name_input).detach();

        let minecraft_versions = FrontendMetadata::request(&data.metadata, MetadataRequest::MinecraftVersionManifest, cx);

        let version_select_state = cx.new(|cx| SelectState::new(VersionList::default(), None, window, cx).searchable(true));
        cx.observe_in(&minecraft_versions, window, |page, versions, window, cx| {
            page.update_minecraft_versions(versions, window, cx);
        }).detach();
        cx.subscribe(&version_select_state, Self::on_minecraft_version_selected).detach();

        let hide_usernames = InterfaceConfig::get(cx).hide_usernames;

        let account_items = cx.new(|cx| {
            let accounts = &data.accounts.read(cx).accounts;
            let mut account_items = Vec::with_capacity(accounts.len());
            let mut selected = None;
            for (index, loop_account) in accounts.iter().enumerate() {
                account_items.push(NamedDropdownItem {
                    name: loop_account.username(hide_usernames),
                    item: loop_account.uuid,
                });
                if let Some(preferred_account) = account && loop_account.uuid == preferred_account {
                    selected = Some(IndexPath::new(index));
                }
           	}

            SelectState::new(NamedDropdown::new(account_items), selected, window, cx).searchable(true)
        });
        cx.observe_in(&data.accounts, window, |page, accounts, window, cx| {
            page.update_account_list(accounts, window, cx);
        }).detach();
        cx.subscribe(&account_items, Self::on_account_selected).detach();

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
            let entry = instance.read(cx);
            page.instance_root_label = PathLabel::new(entry.root_path.clone(), true);
            page.icon = if let Some(raw) = entry.icon.clone() {
                Some(EmbeddedOrRaw::Raw(raw))
            } else if let Some(embedded) = entry.configuration.instance_fallback_icon {
                Some(EmbeddedOrRaw::Embedded(embedded.as_str().into()))
            } else {
                None
            };
            if page.loader_version_select_state.read(cx).selected_index(cx).is_none() {
                let version = entry.configuration.preferred_loader_version.map(|s| s.as_str()).unwrap_or("Latest");
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

        let wrapper_command_input_state = cx.new(|cx| {
            InputState::new(window, cx).auto_grow(1, 8).default_value(wrapper_command.flags)
        });
        cx.subscribe(&wrapper_command_input_state, Self::on_wrapper_command_changed).detach();

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
            account_items,
            loader,
            loader_select_state,
            loader_version_select_state,
            disable_file_syncing,
            sandbox_available,
            sandbox,
            memory_override_enabled: memory.enabled,
            memory_min_input_state,
            memory_max_input_state,
            wrapper_command_enabled: wrapper_command.enabled,
            wrapper_command_input_state,
            jvm_flags_enabled: jvm_flags.enabled,
            jvm_flags_input_state,
            jvm_binary_enabled: jvm_binary.enabled,
            jvm_binary_path: jvm_binary.path.clone().map(|path| PathLabel::new(path, false)),
            override_glfw_enabled: system_libraries.override_glfw,
            override_glfw_path: glfw_path.map(|path| PathLabel::new(path, false)),
            override_openal_enabled: system_libraries.override_openal,
            override_openal_path: openal_path.map(|path| PathLabel::new(path, false)),
            instance_root_label,
            #[cfg(target_os = "linux")]
            use_mangohud: linux_wrapper.use_mangohud,
            #[cfg(target_os = "linux")]
            use_gamemode: linux_wrapper.use_gamemode,
            #[cfg(target_os = "linux")]
            use_discrete_gpu: linux_wrapper.use_discrete_gpu,
            #[cfg(target_os = "linux")]
            disable_gl_threaded_optimizations: linux_wrapper.disable_gl_threaded_optimizations,
            #[cfg(target_os = "linux")]
            mangohud_available: command::is_command_available("mangohud"),
            #[cfg(target_os = "linux")]
            gamemode_available: command::is_command_available("gamemoderun"),
            new_name_change_state: NewNameChangeState::NoChange,
            icon,
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

    fn update_account_list(&mut self, accounts: Entity<AccountEntries>, window: &mut Window, cx: &mut Context<Self>) {
        let hide_usernames = InterfaceConfig::get(cx).hide_usernames;

        let list = accounts.read(cx).accounts
            .iter().map(|account| NamedDropdownItem {
                name: account.username(hide_usernames),
                item: account.uuid,
            }).collect::<Vec<NamedDropdownItem<Uuid>>>();

        self.account_items.update(cx, move |items, cx| {
            items.set_items(NamedDropdown::new(list), window, cx);
        })
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

    pub fn on_account_selected(
    	&mut self,
     	_state: Entity<SelectState<NamedDropdown<Uuid>>>,
     	event: &SelectEvent<NamedDropdown<Uuid>>,
       	_cx: &mut Context<Self>,
    ) {
	   	let SelectEvent::Confirm(value) = event;

		self.backend_handle.send(MessageToBackend::SetInstancePreferredAccount {
			id: self.instance_id,
			account: value.as_ref().map(|value| value.item),
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

    pub fn on_wrapper_command_changed(
        &mut self,
        _: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            self.backend_handle.send(MessageToBackend::SetInstanceWrapperCommand {
                id: self.instance_id,
                wrapper_command: self.get_wrapper_command_configuration(cx)
            });
        }
    }

    fn get_wrapper_command_configuration(&self, cx: &App) -> InstanceWrapperCommandConfiguration {
        let flags = self.wrapper_command_input_state.read(cx).value();

        InstanceWrapperCommandConfiguration {
            enabled: self.wrapper_command_enabled,
            flags: flags.into(),
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
            path: self.jvm_binary_path.as_ref().map(PathLabel::path),
        }
    }

    fn get_system_libraries_configuration(&self) -> InstanceSystemLibrariesConfiguration {
        InstanceSystemLibrariesConfiguration {
            override_glfw: self.override_glfw_enabled,
            glfw: Self::create_lwjgl_library_path(&self.override_glfw_path.as_ref().map(PathLabel::path), &*AUTO_LIBRARY_PATH_GLFW),
            override_openal: self.override_openal_enabled,
            openal: Self::create_lwjgl_library_path(&self.override_openal_path.as_ref().map(PathLabel::path), &*AUTO_LIBRARY_PATH_OPENAL),
        }
    }

    fn create_lwjgl_library_path(path: &Option<Arc<Path>>, auto: &Option<Arc<Path>>) -> LwjglLibraryPath {
        if let Some(path) = path {
            if let Some(auto) = auto && path == auto {
                LwjglLibraryPath::AutoPreferred(path.clone())
            } else {
                LwjglLibraryPath::Explicit(path.clone())
            }
        } else {
            LwjglLibraryPath::Auto
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_wrapper_configuration(&self) -> InstanceLinuxWrapperConfiguration {
        InstanceLinuxWrapperConfiguration {
            use_mangohud: self.use_mangohud,
            use_gamemode: self.use_gamemode,
            use_discrete_gpu: self.use_discrete_gpu,
            disable_gl_threaded_optimizations: self.disable_gl_threaded_optimizations
        }
    }

    pub fn select_file(&mut self, message: &'static str, handle: impl FnOnce(&mut Self, Option<Arc<Path>>) + 'static, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(message.into())
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
        let theme_radius = cx.theme().radius;
        let theme_border = cx.theme().border;

        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child(t::settings::title()));

        let memory_override_enabled = self.memory_override_enabled;
        let wrapper_command_enabled = self.wrapper_command_enabled;
        let jvm_flags_enabled = self.jvm_flags_enabled;
        let jvm_binary_enabled = self.jvm_binary_enabled;

        let icon_element: Option<AnyElement> = self.icon.clone().map(|icon| match icon {
            EmbeddedOrRaw::Embedded(path) => {
                Icon::default().path(path).size_8().min_w_8().min_h_8().into_any_element()
            },
            EmbeddedOrRaw::Raw(data) => {
                let radius = theme_radius;
                let transform = png_render_cache::ImageTransformation::Resize { width: 32, height: 32 };
                png_render_cache::render_with_transform(data, transform, cx)
                    .rounded(radius).size_8().min_w_8().min_h_8().into_any_element()
            },
        });

        let mut basic_content = v_flex()
            .gap_4()
            .size_full()
            .child(crate::labelled(
                t::instance::instance_name(),
                h_flex()
                    .gap_2()
                    .child(Input::new(&self.new_name_input_state))
                    .when(self.new_name_change_state != NewNameChangeState::NoChange, |this| {
                        if self.new_name_change_state == NewNameChangeState::InvalidName {
                            this.child(t::instance::invalid_name())
                        } else {
                            this.child(Button::new("setname").label(t::common::update()).on_click({
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
            )
            .child(crate::labelled(
                t::common::icon(),
                {
                    let mut row = h_flex().gap_2()
                        .child(Button::new("icon").icon(crate::icon::PandoraIcon::Plus).label(t::instance::select_icon()).on_click({
                            let entity = cx.entity();
                            move |_, window, cx| {
                                let entity = entity.clone();
                                crate::modals::select_icon::open_select_icon(Box::new(move |icon, cx| {
                                    cx.update_entity(&entity, |this, _| {
                                        this.icon = Some(icon.clone());
                                        this.backend_handle.send(MessageToBackend::SetInstanceIcon {
                                            id: this.instance_id,
                                            icon: Some(icon),
                                        });
                                    });
                                }), window, cx);
                            }
                        }));
                    if let Some(el) = icon_element {
                        row = row.child(el);
                    }
                    row
                }
            ));

        let mut version_content = v_flex().gap_2();

        match self.version_state {
            TypelessFrontendMetadataResult::Loading => {
                version_content = version_content.child(Skeleton::new().w_full().min_h_8().max_h_8().rounded_md());
            },
            TypelessFrontendMetadataResult::Loaded => {
                version_content = version_content.child(Select::new(&self.version_select_state).w_full());
            },
            TypelessFrontendMetadataResult::Error(ref error) => {
                version_content = version_content.child(format!("{}: {}", t::instance::versions_loading::error(), error))
            },
        }

        version_content = version_content.child(Select::new(&self.loader_select_state).title_prefix(format!("{}: ", t::instance::modloader())).w_full());

        if self.loader != Loader::Vanilla {
            match self.loader_versions_state {
                TypelessFrontendMetadataResult::Loading => {
                    version_content = version_content.child(Skeleton::new().w_full().min_h_8().max_h_8().rounded_md())
                },
                TypelessFrontendMetadataResult::Loaded => {
                    version_content = version_content.child(Select::new(&self.loader_version_select_state).title_prefix(match self.loader {
                        Loader::Fabric => format!("{}: ", t::instance::loader_version(t::modrinth::category::fabric())),
                        Loader::Forge => format!("{}: ", t::instance::loader_version(t::modrinth::category::forge())),
                        Loader::NeoForge => format!("{}: ", t::instance::loader_version(t::modrinth::category::neoforge())),
                        Loader::Vanilla | Loader::Unknown => format!("{}: ", t::instance::loader_version(t::instance::loader())),
                    }).w_full())
                },
                TypelessFrontendMetadataResult::Error(ref error) => {
                    version_content = version_content.child(format!("{}: {}", t::instance::versions_loading::possible_loader_error(), error))
                },
            }
        }

        basic_content = basic_content
            .child(crate::labelled(
                t::instance::version(),
                version_content,
            ))
            .child(crate::labelled(
                t::account::override_account(),
                h_flex()
                .gap_2()
                .child(Select::new(&self.account_items).placeholder("No override").cleanable(true))
            ))
            .child(crate::labelled(
                t::instance::sync::label(),
                Checkbox::new("syncing").label(t::instance::sync::disable_syncing()).checked(self.disable_file_syncing).on_click(cx.listener(|page, value, _, _| {
                    page.disable_file_syncing = *value;
                    page.backend_handle.send(MessageToBackend::SetInstanceDisableFileSyncing {
                        id: page.instance_id,
                        disable_file_syncing: *value
                    });
                }))
            ))
            .child(crate::labelled(
                t::instance::security::label(),
                Checkbox::new("sandbox")
                    .label(t::instance::security::sandbox())
                    .disabled(!self.sandbox && !self.sandbox_available)
                    .tooltip(if self.sandbox_available {
                        "Sandbox the instance, preventing access to files and systems it shouldn't have access to"
                    } else {
                        "Cannot sandbox: missing bwrap and xdg-dbus-proxy commands"
                    })
                    .checked(self.sandbox)
                    .on_click(cx.listener(|page, value, _, _| {
                    page.sandbox = *value;
                    page.backend_handle.send(MessageToBackend::SetInstanceSandboxing {
                        id: page.instance_id,
                        sandbox: *value
                    });
                }))
            ));

        let runtime_content = v_flex()
            .gap_4()
            .size_full()
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("memory").label(t::instance::memory()).checked(memory_override_enabled).on_click(cx.listener(|page, value, _, cx| {
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
                        .line_height(px(24.0))
                        .child(t::common::min())
                        .child(t::common::max()))
                )
            ).child(v_flex()
                .gap_1()
                .child(Checkbox::new("jvm_flags").label(t::instance::jvm_flags()).checked(jvm_flags_enabled).on_click(cx.listener(|page, value, _, cx| {
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
                .child(Checkbox::new("jvm_binary").label(t::instance::jvm_binary()).checked(jvm_binary_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.jvm_binary_enabled != *value {
                        page.jvm_binary_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                            id: page.instance_id,
                            jvm_binary: page.get_jvm_binary_configuration()
                        });
                        cx.notify();
                    }
                })))
                .child(PathLabel::button_opt(&self.jvm_binary_path, "select_jvm_binary").disabled(!jvm_binary_enabled).on_click(cx.listener(|this, _, window, cx| {
                    this.select_file(t::instance::select_jvm_binary(), |this, path| {
                        this.jvm_binary_path = path.map(|path| PathLabel::new(path, false));
                        this.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                            id: this.instance_id,
                            jvm_binary: this.get_jvm_binary_configuration()
                        });
                    }, window, cx);
                })))
            )
            .child(v_flex()
                .gap_1()
                .child(Checkbox::new("system_glfw").label(t::instance::glfw_lib()).checked(self.override_glfw_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.override_glfw_enabled != *value {
                        page.override_glfw_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: page.instance_id,
                            system_libraries: page.get_system_libraries_configuration()
                        });
                        cx.notify();
                    }
                })))
                .child(PathLabel::button_opt(&self.override_glfw_path, "select_glfw").disabled(!self.override_glfw_enabled).on_click(cx.listener(|this, _, window, cx| {
                    this.select_file(t::instance::select_glfw_lib(), |this, path| {
                        this.override_glfw_path = path.map(|path| PathLabel::new(path, false));
                        this.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: this.instance_id,
                            system_libraries: this.get_system_libraries_configuration()
                        });
                    }, window, cx);
                })))
            ).child(v_flex()
                .gap_1()
                .child(Checkbox::new("system_openal").label(t::instance::openal_lib()).checked(self.override_openal_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.override_openal_enabled != *value {
                        page.override_openal_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: page.instance_id,
                            system_libraries: page.get_system_libraries_configuration()
                        });
                        cx.notify();

                    }
                })))
                .child(PathLabel::button_opt(&self.override_openal_path, "select_openal").disabled(!self.override_openal_enabled).on_click(cx.listener(|this, _, window, cx| {
                    this.select_file(t::instance::select_openal_lib(), |this, path| {
                        this.override_openal_path = path.map(|path| PathLabel::new(path, false));
                        this.backend_handle.send(MessageToBackend::SetInstanceSystemLibraries {
                            id: this.instance_id,
                            system_libraries: this.get_system_libraries_configuration()
                        });
                    }, window, cx);
                })))
            ).child(v_flex()
                .gap_1()
                .child(Checkbox::new("wrapper_command").label(t::instance::wrapper_command()).checked(wrapper_command_enabled).on_click(cx.listener(|page, value, _, cx| {
                    if page.wrapper_command_enabled != *value {
                        page.wrapper_command_enabled = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceWrapperCommand {
                            id: page.instance_id,
                            wrapper_command: page.get_wrapper_command_configuration(cx)
                        });
                        cx.notify();
                    }
                })))
                .child(Input::new(&self.wrapper_command_input_state).disabled(!wrapper_command_enabled))
            );

        #[cfg(target_os = "linux")]
        let runtime_content = runtime_content.child(v_flex()
            .gap_1()
            .child(t::instance::linux::label())
            .child(Checkbox::new("use_mangohud")
                .label(t::instance::linux::use_mangohud())
                .checked(self.use_mangohud && self.mangohud_available)
                .disabled(!self.mangohud_available)
                .on_click(cx.listener(|page, value, _, cx| {
                    if page.use_mangohud != *value {
                        page.use_mangohud = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceLinuxWrapper {
                            id: page.instance_id,
                            linux_wrapper: page.get_linux_wrapper_configuration()
                        });
                        cx.notify();
                    }
                })))
            .child(Checkbox::new("use_gamemode")
                .label(t::instance::linux::use_gamemode())
                .checked(self.use_gamemode && self.gamemode_available)
                .disabled(!self.gamemode_available)
                .on_click(cx.listener(|page, value, _, cx| {
                    if page.use_gamemode != *value {
                        page.use_gamemode = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceLinuxWrapper {
                            id: page.instance_id,
                            linux_wrapper: page.get_linux_wrapper_configuration()
                        });
                        cx.notify();
                    }
                })))
            .child(Checkbox::new("use_discrete_gpu")
                .label(t::instance::linux::use_discrete_gpu())
                .checked(self.use_discrete_gpu)
                .on_click(cx.listener(|page, value, _, cx| {
                    if page.use_discrete_gpu != *value {
                        page.use_discrete_gpu = *value;
                        page.backend_handle.send(MessageToBackend::SetInstanceLinuxWrapper {
                            id: page.instance_id,
                            linux_wrapper: page.get_linux_wrapper_configuration()
                        });
                        cx.notify();
                    }
                })))
            .child(Checkbox::new("disable_gl_threaded_optimizations")
                .label(t::instance::linux::disable_gl_threaded_optimizations())
                .checked(self.disable_gl_threaded_optimizations)
                .on_click(cx.listener(|page, value, _, cx| {
                    if page.disable_gl_threaded_optimizations != *value {
                        page.disable_gl_threaded_optimizations = *value;
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
            .child(crate::labelled(
                "Instance Folder (click to relocate)",
                self.instance_root_label.button("relocate").on_click({
                    let instance = self.instance.clone();
                    let backend_handle = self.backend_handle.clone();
                    move |_: &ClickEvent, _, cx| {
                        let instance = instance.read(cx);
                        let id = instance.id;

                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                            files: false,
                            directories: true,
                            multiple: false,
                            prompt: Some("Select empty directory".into()),
                        });
                        let backend_handle = backend_handle.clone();
                        cx.spawn(async move |_| {
                            let Ok(Ok(Some(mut paths))) = receiver.await else {
                                return;
                            };
                            if paths.is_empty() {
                                return;
                            }
                            backend_handle.send(MessageToBackend::RelocateInstance { id, path: paths.swap_remove(0) });
                        }).detach();
                    }
                })
            ))
            .child(Button::new("shortcut").label(t::instance::create_shortcut()).overflow_x_hidden().success().on_click({
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
            .child(Button::new("delete").label(t::instance::delete()).overflow_x_hidden().danger().on_click({
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

        let sections = HorizontalSections::new()
            .size_full()
            .p_4()
            .gap_8()
            .child(basic_content)
            .child(runtime_content)
            .child(actions_content);

        v_flex()
            .p_4()
            .size_full()
            .child(header)
            .child(div()
                .size_full()
                .border_1()
                .rounded(theme_radius)
                .border_color(theme_border)
                .child(sections)
            )
    }
}
