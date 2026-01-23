use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use bridge::{handle::BackendHandle, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, IconName, IndexPath, Selectable, WindowExt,
    alert::Alert,
    button::{Button, ButtonGroup, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    select::{Select, SelectDelegate, SelectItem, SelectState},
    skeleton::Skeleton,
    table::{Table, TableState},
    v_flex,
};
use schema::{loader::Loader, version_manifest::{MinecraftVersionManifest, MinecraftVersionType}};

use crate::{
    component::{instance_list::InstanceList, page_path::PagePath},
    entity::{DataEntities, instance::InstanceEntries, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult}},
    ui,
};

pub struct InstancesPage {
    instance_table: Entity<TableState<InstanceList>>,

    metadata: Entity<FrontendMetadata>,
    instances: Entity<InstanceEntries>,

    backend_handle: BackendHandle,
}

impl InstancesPage {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance_table = InstanceList::create_table(data, window, cx);

        Self {
            instance_table,
            metadata: data.metadata.clone(),
            instances: data.instances.clone(),
            backend_handle: data.backend_handle.clone(),
        }
    }
}

impl Render for InstancesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let create_instance = Button::new("create_instance")
            .success()
            .icon(IconName::Plus)
            .label("Create Instance")
            .on_click(cx.listener(|this, _, window, cx| {
                this.show_create_instance_modal(window, cx);
            }));

        ui::page(cx, h_flex().gap_8().child("Instances").child(create_instance))
            .child(Table::new(&self.instance_table).bordered(false))
    }
}

#[derive(Default)]
pub struct VersionList {
    pub versions: Vec<SharedString>,
    pub matched_versions: Vec<SharedString>,
}

impl SelectDelegate for VersionList {
    type Item = SharedString;

    fn items_count(&self, _section: usize) -> usize {
        self.matched_versions.len()
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.matched_versions.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: gpui_component::select::SelectItem<Value = V>,
        V: PartialEq,
    {
        for (ix, item) in self.matched_versions.iter().enumerate() {
            if item.value() == value {
                return Some(IndexPath::default().row(ix));
            }
        }

        None
    }

    fn perform_search(&mut self, query: &str, _window: &mut Window, _: &mut Context<SelectState<Self>>) -> Task<()> {
        let lower_query = query.to_lowercase();

        self.matched_versions = self
            .versions
            .iter()
            .filter(|item| item.to_lowercase().starts_with(&lower_query))
            .cloned()
            .collect();

        Task::ready(())
    }
}

impl InstancesPage {
    pub fn show_create_instance_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected_loader = Arc::new(AtomicUsize::new(0));
        let loaded_versions = Arc::new(AtomicBool::new(false));
        let error_loading_versions = Arc::new(RwLock::new(None));
        let show_snapshots = Arc::new(AtomicBool::new(false));
        let name_invalid = Arc::new(AtomicBool::new(false));

        let instance_names: Arc<[SharedString]> =
            self.instances.read(cx).entries.iter().map(|(_, v)| v.read(cx).name.clone()).collect();

        let minecraft_version_dropdown =
            cx.new(|cx| SelectState::new(VersionList::default(), None, window, cx).searchable(true));

        let unnamed_instance_name = SharedString::new_static("Unnamed Instance");

        let name_input_state = cx.new(|cx| InputState::new(window, cx).placeholder(unnamed_instance_name.clone()));

        let _name_input_subscription = {
            let name_invalid = Arc::clone(&name_invalid);
            let instance_names = Arc::clone(&instance_names);
            cx.subscribe_in(&name_input_state, window, move |_, input_state, _: &InputEvent, _, cx| {
                let text = input_state.read(cx).value();

                if !text.as_str().is_empty() {
                    if !crate::is_valid_instance_name(text.as_str()) {
                        name_invalid.store(true, Ordering::Relaxed);
                        return;
                    }
                }

                name_invalid.store(instance_names.contains(&text), Ordering::Relaxed);
            })
        };

        let versions = FrontendMetadata::request(&self.metadata, bridge::meta::MetadataRequest::MinecraftVersionManifest, cx);

        let backend_handle = self.backend_handle.clone();

        let reload_version_dropdown = {
            let loaded_versions = Arc::clone(&loaded_versions);
            let show_snapshots = Arc::clone(&show_snapshots);
            let error_loading_versions = Arc::clone(&error_loading_versions);
            let minecraft_version_dropdown = minecraft_version_dropdown.clone();
            let versions = versions.clone();

            move |window: &mut Window, cx: &mut App| {
                cx.update_entity(&minecraft_version_dropdown, |dropdown, cx| {
                    let result: FrontendMetadataResult<MinecraftVersionManifest> = versions.read(cx).result();
                    let (versions, latest) = match result {
                        FrontendMetadataResult::Loading => {
                            loaded_versions.store(false, Ordering::Relaxed);
                            (Vec::new(), None)
                        },
                        FrontendMetadataResult::Error(error) => {
                            loaded_versions.store(false, Ordering::Relaxed);
                            *error_loading_versions.write().unwrap() = Some(error);
                            (Vec::new(), None)
                        },
                        FrontendMetadataResult::Loaded(manifest) => {
                            loaded_versions.store(true, Ordering::Relaxed);
                            *error_loading_versions.write().unwrap() = None;

                            let versions: Vec<SharedString> = if show_snapshots.load(Ordering::Relaxed) {
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
            }
        };

        (reload_version_dropdown)(window, cx);

        let subscription = {
            let window_handle = window.window_handle();
            let reload_version_dropdown = reload_version_dropdown.clone();
            cx.observe(&versions, move |_, _, cx| {
                let _ = window_handle.update(cx, |_, window, cx| {
                    (reload_version_dropdown)(window, cx);
                });
            })
        };

        struct FallbackNameInfo {
            original: SharedString,
            actual: SharedString,
        }
        let fallback_name_info = Arc::new(Mutex::new(FallbackNameInfo {
            original: unnamed_instance_name.clone(),
            actual: unnamed_instance_name.clone(),
        }));

        let metadata = self.metadata.clone();

        window.open_dialog(cx, move |modal, window, cx| {
            let _ = &subscription;
            let _ = &_name_input_subscription;

            name_input_state.update(cx, |input_state, cx| {
                let selected = minecraft_version_dropdown
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .unwrap_or(unnamed_instance_name.clone());

                let mut fallback_name_info = fallback_name_info.lock().unwrap();

                if fallback_name_info.original != selected {
                    fallback_name_info.original = selected.clone();

                    if instance_names.contains(&selected) {
                        for i in 1..10 {
                            let new_name = SharedString::from(format!("{}-{}", selected, i));
                            if !instance_names.contains(&new_name) {
                                fallback_name_info.actual = new_name.clone();
                                input_state.set_placeholder(new_name, window, cx);
                                return;
                            }
                        }
                    }

                    fallback_name_info.actual = selected.clone();
                    input_state.set_placeholder(selected, window, cx);
                }
            });

            if let Some(error) = error_loading_versions.read().unwrap().as_ref() {
                let error_widget = Alert::new("error", format!("{}", error))
                    .icon(IconName::CircleX)
                    .title("Error loading Minecraft versions");

                let error_loading_versions = Arc::clone(&error_loading_versions);
                let metadata = metadata.clone();
                let reload_button =
                    Button::new("reload-versions")
                        .primary()
                        .label("Reload Versions")
                        .on_click(move |_, _, cx| {
                            *error_loading_versions.write().unwrap() = None;
                            FrontendMetadata::force_reload(&metadata, bridge::meta::MetadataRequest::MinecraftVersionManifest, cx);
                        });

                return modal
                    .confirm()
                    .title("Create Instance")
                    .child(v_flex().gap_3().child(error_widget).child(reload_button));
            }

            let selected_loader_value = match selected_loader.load(Ordering::Relaxed) {
                0 => Loader::Vanilla,
                1 => Loader::Fabric,
                2 => Loader::Forge,
                3 => Loader::NeoForge,
                _ => unreachable!(),
            };

            let version_dropdown;
            let show_snapshots_button;
            let loader_button_group;

            if !loaded_versions.load(Ordering::Relaxed) {
                version_dropdown = Select::new(&minecraft_version_dropdown)
                    .w_full()
                    .disabled(true)
                    .placeholder("Loading Minecraft Versions...");
                show_snapshots_button = Skeleton::new().w_full().min_h_4().max_h_4().rounded_md().into_any_element();
                loader_button_group = Skeleton::new().w_full().min_h_8().max_h_8().rounded_md().into_any_element();
            } else {
                let reload_version_dropdown = reload_version_dropdown.clone();
                let selected_loader = selected_loader.clone();

                let show_snapshots = Arc::clone(&show_snapshots);
                let show_snapshots_value = show_snapshots.load(Ordering::Relaxed);

                version_dropdown = Select::new(&minecraft_version_dropdown).title_prefix("Minecraft Version: ");
                show_snapshots_button = Checkbox::new("show_snapshots")
                    .checked(show_snapshots_value)
                    .label("Show Snapshots")
                    .on_click(move |show, window, cx| {
                        show_snapshots.store(*show, Ordering::Relaxed);
                        (reload_version_dropdown)(window, cx);
                    })
                    .into_any_element();
                loader_button_group = ButtonGroup::new("loader")
                    .outline()
                    .h_full()
                    .child(
                        Button::new("loader-vanilla")
                            .label("Vanilla")
                            .selected(selected_loader_value == Loader::Vanilla),
                    )
                    .child(
                        Button::new("loader-fabric")
                            .label("Fabric")
                            .selected(selected_loader_value == Loader::Fabric),
                    )
                    .child(
                        Button::new("loader-forge")
                            .label("Forge")
                            .selected(selected_loader_value == Loader::Forge),
                    )
                    .child(
                        Button::new("loader-neoforge")
                            .label("NeoForge")
                            .selected(selected_loader_value == Loader::NeoForge),
                    )
                    .on_click(move |selected, _, _| {
                        match selected.first() {
                            Some(0) => selected_loader.store(0, Ordering::Relaxed),
                            Some(1) => selected_loader.store(1, Ordering::Relaxed),
                            Some(2) => selected_loader.store(2, Ordering::Relaxed),
                            Some(3) => selected_loader.store(3, Ordering::Relaxed),
                            _ => {},
                        };
                    })
                    .into_any_element();
            };

            let minecraft_version_dropdown = minecraft_version_dropdown.clone();

            let name_is_invalid = name_invalid.load(Ordering::Relaxed);

            let content = v_flex()
                .gap_3()
                .child(crate::labelled(
                    "Name",
                    Input::new(&name_input_state).when(name_is_invalid, |this| this.border_color(cx.theme().danger)),
                ))
                .child(crate::labelled("Version", v_flex().gap_2().child(version_dropdown).child(show_snapshots_button)))
                .child(crate::labelled("Modloader", loader_button_group));

            let text_input_state = name_input_state.clone();
            let backend_handle = backend_handle.clone();
            let fallback_name_info = Arc::clone(&fallback_name_info);

            modal
                .footer(move |ok, cancel, window, cx| {
                    if name_is_invalid {
                        vec![
                            cancel(window, cx),
                            div().child(ok(window, cx)).opacity(0.5).into_any_element(),
                        ]
                    } else {
                        vec![cancel(window, cx), ok(window, cx)]
                    }
                })
                .overlay_closable(false)
                .title("Create Instance")
                .on_ok(move |_, _, cx| {
                    if name_is_invalid {
                        return false;
                    }
                    let Some(selected_version) = minecraft_version_dropdown.read(cx).selected_value().cloned() else {
                        return false;
                    };

                    let mut name = text_input_state.read(cx).value().clone();
                    if name.is_empty() {
                        let fallback_name_info = fallback_name_info.lock().unwrap();
                        name = fallback_name_info.actual.clone();
                    }

                    backend_handle.send(MessageToBackend::CreateInstance {
                        name: name.as_str().into(),
                        version: selected_version.as_str().into(),
                        loader: selected_loader_value,
                    });

                    true
                })
                .child(content)
        });
    }
}
