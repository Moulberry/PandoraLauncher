use std::{path::{Path, PathBuf}, sync::Arc};

use bridge::{
    handle::BackendHandle, install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::{InstanceContentSummary, InstanceID}, message::{BridgeDataLoadState, MessageToBackend}, serial::AtomicOptionSerial
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, IndexPath, Sizable, WindowExt, button::{Button, ButtonVariants}, h_flex, input::SelectAll, list::ListState, notification::{Notification, NotificationType}, select::{Select, SelectEvent, SelectState}, switch::Switch, v_flex
};
use schema::{content::ContentSource, curseforge::CurseforgeClassId, loader::Loader, modrinth::ModrinthProjectType};
use ustr::Ustr;

use crate::{component::{content_list::ContentListDelegate, named_dropdown::{NamedDropdown, NamedDropdownItem}}, entity::instance::InstanceEntry, interface_config::{InstanceContentSortKey, InterfaceConfig}, root, ui::PageType};

pub struct InstanceModsSubpage {
    instance: InstanceID,
    instance_loader: Loader,
    instance_version: Ustr,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    mods_state: BridgeDataLoadState,
    mod_list: Entity<ListState<ContentListDelegate>>,
    mods: Entity<Arc<[InstanceContentSummary]>>,
    sort_dropdown: Entity<SelectState<NamedDropdown<InstanceContentSortKey>>>,
    load_serial: AtomicOptionSerial,
    _add_from_file_task: Option<Task<()>>,
}

impl InstanceModsSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let instance = instance.read(cx);
        let instance_loader = instance.configuration.loader;
        let instance_version = instance.configuration.minecraft_version;
        let instance_id = instance.id;
        let instance_name = instance.name.clone();

        let mods_state = instance.mods_state.clone();

        let sort_key = InterfaceConfig::get(cx).instance_mods_sort_key;
        let enabled_first = InterfaceConfig::get(cx).instance_mods_enabled_first;

        let mut mods_list_delegate = ContentListDelegate::new(instance_id, backend_handle.clone(), instance_loader, instance_version, sort_key, enabled_first);
        mods_list_delegate.set_content(instance.mods.read(cx));

        let mods = instance.mods.clone();

        let sort_dropdown = cx.new(|cx| {
            let items = [
                InstanceContentSortKey::Filename,
                InstanceContentSortKey::Name,
                InstanceContentSortKey::ModifiedTime,
                InstanceContentSortKey::FileSize,
            ].into_iter().map(|key| {
                NamedDropdownItem { name: key.name(), item: key }
            }).collect::<Vec<_>>();

            let current = InterfaceConfig::get(cx).instance_mods_sort_key;
            let row = items.iter().position(|v| v.item == current).unwrap_or(0);
            SelectState::new(NamedDropdown::new(items), Some(IndexPath::new(row)), window, cx)
        });

        let mods_for_observe = mods.clone();
        let mod_list = cx.new(move |cx| {
            cx.observe(&mods_for_observe, |list: &mut ListState<ContentListDelegate>, mods, cx| {
                let actual_mods = mods.read(cx);
                list.delegate_mut().set_content(actual_mods);
                cx.notify();
            }).detach();

            ListState::new(mods_list_delegate, window, cx).selectable(false).searchable(true)
        });

        cx.subscribe(&sort_dropdown, |this, _, event: &SelectEvent<NamedDropdown<InstanceContentSortKey>>, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };

            let sort_key = value.item;
            let enabled_first = InterfaceConfig::get(cx).instance_mods_enabled_first;
            InterfaceConfig::get_mut(cx).instance_mods_sort_key = sort_key;

            let mods_snapshot = this.mods.read(cx).clone();
            let mod_list = this.mod_list.clone();
            cx.update_entity(&mod_list, |list, cx| {
                list.delegate_mut().set_sort_options(sort_key, enabled_first);
                list.delegate_mut().set_content(mods_snapshot.as_ref());
                cx.notify();
            });
            cx.notify();
        }).detach();

        Self {
            instance: instance_id,
            instance_loader,
            instance_version,
            instance_name,
            backend_handle,
            mods_state,
            mod_list,
            mods,
            sort_dropdown,
            load_serial: AtomicOptionSerial::default(),
            _add_from_file_task: None,
        }
    }

    fn install_paths(&self, paths: &[PathBuf], window: &mut Window, cx: &mut App) {
        let content_install = ContentInstall {
            target: InstallTarget::Instance(self.instance),
            loader_hint: self.instance_loader,
            version_hint: Some(self.instance_version.into()),
            files: paths.into_iter().filter_map(|path| {
                Some(ContentInstallFile {
                    replace_old: None,
                    path: bridge::install::ContentInstallPath::Raw(Path::new("mods").join(path.file_name()?).into()),
                    download: ContentDownload::File { path: path.clone() },
                    content_source: ContentSource::Manual,
                })
            }).collect(),
        };
        crate::root::start_install(content_install, &self.backend_handle, window, cx);
    }
}

impl Render for InstanceModsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        self.mods_state.set_observed();
        if self.mods_state.should_load() {
            self.backend_handle.send_with_serial(MessageToBackend::RequestLoadMods { id: self.instance }, &self.load_serial);
        }

        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child(t::instance::content::mods()))
            .child(Button::new("update").label(t::instance::content::update::check::label(false)).success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance_id = self.instance;
                move |_, window, cx| {
                    crate::root::start_update_check(instance_id, &backend_handle, window, cx);
                }
            }))
            .child(Button::new("addmr").label(t::instance::content::install::from_modrinth()).success().compact().small().on_click({
                let instance_name = self.instance_name.clone();
                move |_, window, cx| {
                    let page = crate::ui::PageType::Modrinth { installing_for: Some(instance_name.clone()) };
                    InterfaceConfig::get_mut(cx).modrinth_page_project_type = ModrinthProjectType::Mod;
                    let path = &[PageType::Instances, PageType::InstancePage { name: instance_name.clone() }];
                    root::switch_page(page, path, window, cx);
                }
            }))
            .child(Button::new("addcf").label(t::instance::content::install::from_curseforge()).success().compact().small().on_click({
                let instance_name = self.instance_name.clone();
                move |_, window, cx| {
                    let page = crate::ui::PageType::Curseforge { installing_for: Some(instance_name.clone()) };
                    InterfaceConfig::get_mut(cx).curseforge_page_class_id = CurseforgeClassId::Mod;
                    let path = &[PageType::Instances, PageType::InstancePage { name: instance_name.clone() }];
                    root::switch_page(page, path, window, cx);
                }
            }))
            .child(Button::new("addfile").label(t::instance::content::install::from_file()).success().compact().small().on_click({
                cx.listener(move |this, _, window, cx| {
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: true,
                        prompt: Some(t::instance::content::install::select_mods().into())
                    });

                    let entity = cx.entity();
                    let add_from_file_task = window.spawn(cx, async move |cx| {
                        let Ok(result) = receiver.await else {
                            return;
                        };
                        _ = cx.update_window_entity(&entity, move |this, window, cx| {
                            match result {
                                Ok(Some(paths)) => {
                                    this.install_paths(&paths, window, cx);
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
                    this._add_from_file_task = Some(add_from_file_task);
                })
            }));

        let filter_bar_controls = h_flex()
            .gap_3()
            .items_center()
            .child(div().child(Select::new(&self.sort_dropdown).small().title_prefix("Sort: ".to_string())))
            .child(h_flex().gap_1()
                .child(div().text_sm().child("Enabled first"))
                .child(Switch::new("mods_enabled_first")
                    .checked(InterfaceConfig::get(cx).instance_mods_enabled_first)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        let sort_key = InterfaceConfig::get(cx).instance_mods_sort_key;
                        let enabled_first = *checked;
                        InterfaceConfig::get_mut(cx).instance_mods_enabled_first = enabled_first;

                        let mods_snapshot = this.mods.read(cx).clone();
                        let mod_list = this.mod_list.clone();
                        cx.update_entity(&mod_list, |list, cx| {
                            list.delegate_mut().set_sort_options(sort_key, enabled_first);
                            list.delegate_mut().set_content(mods_snapshot.as_ref());
                            cx.notify();
                        });
                        cx.notify();
                    }))
                )
            )
            .absolute()
            .top(px(6.0))
            .right(px(12.0));

        v_flex().p_4().size_full()
            .child(header)
            .child(div()
                .id("mod-list-area")
                .relative()
                .drag_over(|style, _: &ExternalPaths, _, cx| {
                    style.bg(cx.theme().accent)
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    this.install_paths(paths.paths(), window, cx);
                }))
                .size_full()
                .border_1()
                .rounded(theme.radius)
                .border_color(theme.border)
                .child(self.mod_list.clone())
                .child(filter_bar_controls)
                .on_click({
                    let mod_list = self.mod_list.clone();
                    move |_, _, cx| {
                        cx.update_entity(&mod_list, |list, cx| {
                            list.delegate_mut().clear_selection();
                            cx.notify();
                        })
                    }
                })
                .key_context("Input")
                .on_action({
                    let mod_list = self.mod_list.clone();
                    move |_: &SelectAll, _, cx| {
                        cx.update_entity(&mod_list, |list, cx| {
                            list.delegate_mut().select_all();
                            cx.notify();
                        })
                    }
                }),
        )
    }
}
