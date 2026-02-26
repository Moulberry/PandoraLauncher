use std::sync::Arc;

use bridge::{handle::BackendHandle, message::{EmbeddedOrRaw, MessageToBackend}};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IconName, Selectable, WindowExt, alert::Alert, button::{Button, ButtonGroup, ButtonVariants}, checkbox::Checkbox, dialog::{Dialog, DialogFooter}, h_flex, input::{Input, InputEvent, InputState}, select::{Select, SelectState}, skeleton::Skeleton, v_flex
};
use schema::{loader::Loader, version_manifest::{MinecraftVersionManifest, MinecraftVersionType}};

use crate::{entity::{instance::InstanceEntries, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState}}, interface_config::InterfaceConfig, pages::instances_page::VersionList, ts};

struct CreateInstanceModalState {
    metadata: Entity<FrontendMetadata>,
    versions: Entity<FrontendMetadataState>,
    backend_handle: BackendHandle,
    minecraft_version_dropdown: Entity<SelectState<VersionList>>,
    name_input_state: Entity<InputState>,
    selected_loader: Loader,
    loaded_versions: bool,
    error_loading_versions: Option<SharedString>,
    name_invalid: bool,
    instance_names: Arc<[SharedString]>,
    original_fallback_name: SharedString,
    unique_fallback_name: SharedString,
    icon: Option<EmbeddedOrRaw>,
    _versions_updated_subscription: Subscription,
    _name_input_subscription: Subscription,
    _version_selected_subscription: Subscription,
}

impl CreateInstanceModalState {
    pub fn new(metadata: Entity<FrontendMetadata>, instances: Entity<InstanceEntries>, backend_handle: BackendHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance_names: Arc<[SharedString]> =
            instances.read(cx).entries.iter().map(|(_, v)| v.read(cx).name.clone()).collect();

        let minecraft_version_dropdown =
            cx.new(|cx| SelectState::new(VersionList::default(), None, window, cx).searchable(true));

        let _version_selected_subscription = cx.observe_in(&minecraft_version_dropdown, window, |this, _, window, cx| {
            this.update_fallback_name(window, cx);
        });

        let name_input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(ts!("instance.unnamed"))
        });

        let _name_input_subscription = {
            let instance_names = Arc::clone(&instance_names);
            cx.subscribe_in(&name_input_state, window, move |this, input_state, _: &InputEvent, _, cx| {
                let text = input_state.read(cx).value();

                if !text.as_str().is_empty() {
                    if !crate::is_valid_instance_name(text.as_str()) {
                        this.name_invalid = true;
                        return;
                    }
                }

                this.name_invalid = instance_names.contains(&text);
            })
        };

        let versions = FrontendMetadata::request(&metadata, bridge::meta::MetadataRequest::MinecraftVersionManifest, cx);

        let _versions_updated_subscription = cx.observe_in(&versions, window, move |this, _, window, cx| {
            this.reload_version_dropdown(window, cx);
        });

        let mut this = Self {
            metadata,
            versions,
            backend_handle,
            minecraft_version_dropdown,
            name_input_state,
            selected_loader: Loader::Vanilla,
            loaded_versions: false,
            error_loading_versions: None,
            name_invalid: false,
            instance_names,
            original_fallback_name: Default::default(),
            unique_fallback_name: Default::default(),
            icon: None,
            _versions_updated_subscription,
            _name_input_subscription,
            _version_selected_subscription,
        };

        this.reload_version_dropdown(window, cx);

        this
    }

    pub fn update_fallback_name(&mut self, window: &mut Window, cx: &mut App) {
        let selected = self.minecraft_version_dropdown
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or(ts!("instance.unnamed"));

        if self.original_fallback_name != selected {
            self.original_fallback_name = selected.clone();

            if self.instance_names.contains(&selected) {
                for i in 1..10 {
                    let new_name = SharedString::from(format!("{}-{}", selected, i));
                    if !self.instance_names.contains(&new_name) {
                        self.unique_fallback_name = new_name.clone();
                        cx.update_entity(&self.name_input_state, |input_state, cx| {
                            input_state.set_placeholder(new_name, window, cx);
                        });
                        return;
                    }
                }
            }

            self.unique_fallback_name = selected.clone();
            cx.update_entity(&self.name_input_state, |input_state, cx| {
                input_state.set_placeholder(selected, window, cx);
            });
        }
    }

    pub fn reload_version_dropdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.update_entity(&self.minecraft_version_dropdown, |dropdown, cx| {
            let result: FrontendMetadataResult<MinecraftVersionManifest> = self.versions.read(cx).result();
            let (versions, latest) = match result {
                FrontendMetadataResult::Loading => {
                    self.loaded_versions = false;
                    self.error_loading_versions = None;
                    (Vec::new(), None)
                },
                FrontendMetadataResult::Error(error) => {
                    self.loaded_versions = false;
                    self.error_loading_versions = Some(error);
                    (Vec::new(), None)
                },
                FrontendMetadataResult::Loaded(manifest) => {
                    self.loaded_versions = true;
                    self.error_loading_versions = None;

                    let show_snapshots = InterfaceConfig::get(cx).show_snapshots_in_create_instance;
                    let versions: Vec<SharedString> = if show_snapshots {
                        manifest.versions.iter().map(|v| SharedString::from(v.id.as_str())).collect()
                    } else {
                        manifest
                            .versions
                            .iter()
                            .filter(|v| !matches!(v.r#type, MinecraftVersionType::Snapshot))
                            .map(|v| SharedString::from(v.id.as_str()))
                            .collect()
                    };

                    (versions, Some(SharedString::from(manifest.latest.release.as_str())))
                },
            };

            let mut to_select = None;

            if let Some(last_selected) = dropdown.selected_value().cloned()
                && versions.contains(&last_selected)
            {
                to_select = Some(last_selected);
            }

            if to_select.is_none()
                && let Some(latest) = latest
                && versions.contains(&latest)
            {
                to_select = Some(latest);
            }

            if to_select.is_none() {
                to_select = versions.first().cloned();
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

        if self.loaded_versions {
            self.update_fallback_name(window, cx);
        }
    }

    pub fn render(&mut self, modal: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        if let Some(error) = self.error_loading_versions.clone() {
            let error_widget = Alert::new("error", format!("{}", error))
                .icon(IconName::CircleX)
                .title(ts!("instance.versions_loading.error"));

            let metadata = self.metadata.clone();
            let reload_button =
                Button::new("reload-versions")
                    .primary()
                    .label(ts!("instance.versions_loading.reload"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.error_loading_versions = None;
                        FrontendMetadata::force_reload(&metadata, bridge::meta::MetadataRequest::MinecraftVersionManifest, cx);
                    }));

            return modal
                .title(ts!("instance.create"))
                .child(v_flex().gap_3().child(error_widget).child(reload_button))
                .footer(Button::new("ok").label(ts!("common.ok")).on_click(|_, window, cx| window.close_dialog(cx)));
        }

        let version_dropdown;
        let show_snapshots_button;
        let loader_button_group;

        if !self.loaded_versions {
            version_dropdown = Select::new(&self.minecraft_version_dropdown)
                .w_full()
                .disabled(true)
                .placeholder(ts!("instance.versions_loading.game_versions"));
            show_snapshots_button = Skeleton::new().w_full().min_h_4().max_h_4().rounded_md().into_any_element();
            loader_button_group = Skeleton::new().w_full().min_h_8().max_h_8().rounded_md().into_any_element();
        } else {
            version_dropdown = Select::new(&self.minecraft_version_dropdown).title_prefix(format!("{}: ", ts!("instance.mc_version")));
            show_snapshots_button = Checkbox::new("show_snapshots")
                .checked(InterfaceConfig::get(cx).show_snapshots_in_create_instance)
                .label(ts!("instance.show_snapshots"))
                .on_click(cx.listener(move |this, show, window, cx| {
                    InterfaceConfig::get_mut(cx).show_snapshots_in_create_instance = *show;
                    this.reload_version_dropdown(window, cx);
                }))
                .into_any_element();
            loader_button_group = ButtonGroup::new("loader")
                .outline()
                .h_full()
                .child(
                    Button::new("loader-vanilla")
                        .label(ts!("instance.vanilla"))
                        .selected(self.selected_loader == Loader::Vanilla),
                )
                .child(
                    Button::new("loader-fabric")
                        .label(ts!("modrinth.category.fabric"))
                        .selected(self.selected_loader == Loader::Fabric),
                )
                .child(
                    Button::new("loader-forge")
                        .label(ts!("modrinth.category.forge"))
                        .selected(self.selected_loader == Loader::Forge),
                )
                .child(
                    Button::new("loader-neoforge")
                        .label(ts!("modrinth.category.neoforge"))
                        .selected(self.selected_loader == Loader::NeoForge),
                )
                .on_click(cx.listener(move |this, selected: &Vec<usize>, _, _| {
                    match selected.first() {
                        Some(0) => this.selected_loader = Loader::Vanilla,
                        Some(1) => this.selected_loader = Loader::Fabric,
                        Some(2) => this.selected_loader = Loader::Forge,
                        Some(3) => this.selected_loader = Loader::NeoForge,
                        _ => {},
                    };
                }))
                .into_any_element();
        };

        let content = v_flex()
            .gap_3()
            .child(crate::labelled(
                ts!("instance.name"),
                Input::new(&self.name_input_state).when(self.name_invalid, |this| this.border_color(cx.theme().danger)),
            ))
            .child(crate::labelled(ts!("instance.version"), v_flex().gap_2().child(version_dropdown).child(show_snapshots_button)))
            .child(crate::labelled(ts!("instance.modloader"), loader_button_group))
            .child(h_flex().child(Button::new("icon").icon(IconName::Plus).label(ts!("instance.select_icon")).on_click({
                let entity = cx.entity();
                move |_, window, cx| {
                    let entity = entity.clone();
                    crate::modals::select_icon::open_select_icon(Box::new(move |icon, cx| {
                        cx.update_entity(&entity, |this, _| {
                            this.icon = Some(icon);
                        });
                    }), window, cx);
                }
            })));

        let name_is_invalid = self.name_invalid;
        let entity = cx.entity();
        modal
            .overlay_closable(false)
            .title(ts!("instance.create"))
            .child(content)
            .when(name_is_invalid, |modal| {
                modal.footer(h_flex().gap_2().w_full()
                    .child(Button::new("cancel").flex_1().label(ts!("common.cancel"))
                        .on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("ok").flex_1().opacity(0.5).label(ts!("common.ok"))))
            })
            .when(!name_is_invalid, |modal| {
                modal.footer(h_flex().gap_2().w_full()
                    .child(Button::new("cancel").flex_1().label(ts!("common.cancel"))
                        .on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("ok").flex_1().label(ts!("common.ok"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if name_is_invalid {
                                return;
                            }
                            let Some(selected_version) = this.minecraft_version_dropdown.read(cx).selected_value().cloned() else {
                                return;
                            };

                            let mut name = this.name_input_state.read(cx).value().clone();
                            if name.is_empty() {
                                name = this.unique_fallback_name.clone();
                            }

                            this.backend_handle.send(MessageToBackend::CreateInstance {
                                name: name.as_str().into(),
                                version: selected_version.as_str().into(),
                                loader: this.selected_loader,
                                icon: this.icon.clone(),
                            });
                            window.close_dialog(cx);
                        }))))
            })
    }
}

pub fn open_create_instance(
    metadata: Entity<FrontendMetadata>,
    instances: Entity<InstanceEntries>,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let state = cx.new(|cx| {
        CreateInstanceModalState::new(metadata, instances, backend_handle, window, cx)
    });

    window.open_dialog(cx, move |modal, window, cx| {
        cx.update_entity(&state, |state, cx| {
            state.render(modal, window, cx)
        })
    });
}
