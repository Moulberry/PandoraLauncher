use std::{path::{Path, PathBuf}, sync::Arc};

use bridge::{
    handle::BackendHandle, install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::{ContentFolder, InstanceContentSummary, InstanceID}
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, IndexPath, Sizable, WindowExt, button::{Button, ButtonVariants, DropdownButton}, h_flex, input::SelectAll, list::{List, ListState}, menu::PopupMenuItem, notification::{Notification, NotificationType}, select::{Select, SelectEvent, SelectState}, switch::Switch, v_flex
};
use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use schema::{content::{ContentInstallReason, ContentSource}, curseforge::CurseforgeClassId, loader::Loader, modrinth::ModrinthProjectType};
use ustr::Ustr;

use crate::{component::{content_list::ContentListDelegate, named_dropdown::{NamedDropdown, NamedDropdownItem}}, entity::{DataEntities, instance::{ContentStates, InstanceEntry}}, interface_config::{InstanceContentSortKey, InterfaceConfig, PreferredAddContentSource}, root, ui::PageType};

pub struct InstanceContentSubpage {
    content_type: ContentType,
    instance: InstanceID,
    instance_loader: Loader,
    instance_version: Ustr,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    content_states: ContentStates,
    content_list: Entity<ListState<ContentListDelegate>>,
    updating: Arc<Mutex<FxHashSet<u64>>>,
    content: Entity<Option<Arc<[InstanceContentSummary]>>>,
    sort_dropdown: Entity<SelectState<NamedDropdown<InstanceContentSortKey>>>,

    needs_update_check: bool,
    update_count: usize,

    _add_from_file_task: Option<Task<()>>,
}

#[derive(Clone, Copy)]
pub enum ContentType {
    Mods,
    ResourcePacks,
    Shaders,
}

impl ContentType {
    fn content_folder(self) -> ContentFolder {
        match self {
            ContentType::Mods => ContentFolder::Mods,
            ContentType::ResourcePacks => ContentFolder::ResourcePacks,
            ContentType::Shaders => ContentFolder::Shaders,
        }
    }

    fn title(self) -> &'static str {
        match self {
            ContentType::Mods => t::instance::content::mods(),
            ContentType::ResourcePacks => t::instance::content::resourcepacks(),
            ContentType::Shaders => t::instance::content::shaders(),
        }
    }

    fn install_select(self) -> &'static str {
        match self {
            ContentType::Mods => t::instance::content::install::select_mods(),
            ContentType::ResourcePacks => t::instance::content::install::select_resourcepacks(),
            ContentType::Shaders => t::instance::content::install::select_shaders(),
        }
    }

    fn modrinth_project_type(self) -> ModrinthProjectType {
        match self {
            ContentType::Mods => ModrinthProjectType::Mod,
            ContentType::ResourcePacks => ModrinthProjectType::Resourcepack,
            ContentType::Shaders => ModrinthProjectType::Shader,
        }
    }

    fn curseforge_class_id(self) -> CurseforgeClassId {
        match self {
            ContentType::Mods => CurseforgeClassId::Mod,
            ContentType::ResourcePacks => CurseforgeClassId::Resourcepack,
            ContentType::Shaders => CurseforgeClassId::Shader,
        }
    }

    fn valid_sort_modes(self) -> &'static [InstanceContentSortKey] {
        match self {
            ContentType::Mods => &[
                InstanceContentSortKey::Name,
                InstanceContentSortKey::ModId,
                InstanceContentSortKey::Filename,
                InstanceContentSortKey::ModifiedTime,
                InstanceContentSortKey::FileSize,
            ],
            ContentType::ResourcePacks | ContentType::Shaders => &[
                InstanceContentSortKey::Filename,
                InstanceContentSortKey::ModifiedTime,
                InstanceContentSortKey::FileSize,
            ],
        }
    }

    fn sort_key(self, config: &InterfaceConfig) -> InstanceContentSortKey {
        match self {
            ContentType::Mods => config.instance_mods_sort_key,
            ContentType::ResourcePacks => config.instance_resourcepacks_sort_key,
            ContentType::Shaders => config.instance_shaders_sort_key,
        }
    }

    fn sort_enabled_first(self, config: &InterfaceConfig) -> bool {
        match self {
            ContentType::Mods => config.instance_mods_sort_enabled_first,
            ContentType::ResourcePacks => config.instance_resourcepacks_sort_enabled_first,
            ContentType::Shaders => config.instance_shaders_sort_enabled_first,
        }
    }

    fn set_sort_key(self, config: &mut InterfaceConfig, value: InstanceContentSortKey) {
        match self {
            ContentType::Mods => config.instance_mods_sort_key = value,
            ContentType::ResourcePacks => config.instance_resourcepacks_sort_key = value,
            ContentType::Shaders => config.instance_shaders_sort_key = value,
        }
    }

    fn set_sort_enabled_first(self, config: &mut InterfaceConfig, value: bool) {
        match self {
            ContentType::Mods => config.instance_mods_sort_enabled_first = value,
            ContentType::ResourcePacks => config.instance_resourcepacks_sort_enabled_first = value,
            ContentType::Shaders => config.instance_shaders_sort_enabled_first = value,
        }
    }
}

impl InstanceContentSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        content_type: ContentType,
        data: &DataEntities,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let instance = instance.read(cx);
        let instance_loader = instance.configuration.loader;
        let instance_version = instance.configuration.minecraft_version;
        let instance_id = instance.id;
        let instance_name = instance.name.clone();
        let content_states = instance.content_states.clone();

        let content_folder = content_type.content_folder();
        let content = instance.content[content_folder].clone();

        let config = InterfaceConfig::get(cx);
        let mut sort_key = content_type.sort_key(config);
        let enabled_first = content_type.sort_enabled_first(config);

        let valid_sort_modes = content_type.valid_sort_modes();
        if !valid_sort_modes.contains(&sort_key) {
            sort_key = valid_sort_modes[0];
        }

        let updating = Arc::new(Mutex::new(FxHashSet::default()));
        let mut content_list_delegate = ContentListDelegate::new(instance_id, data, instance_loader, instance_version, sort_key, enabled_first, updating.clone());

        let (needs_update_check, update_count) = if let Some(new_content) = content.read(cx) {
            content_list_delegate.set_content(new_content);
            calculate_update_count(instance_loader, instance_version, new_content)
        } else {
            (false, 0)
        };

        let sort_dropdown = cx.new(|cx| {
            let items = valid_sort_modes.iter().map(|key| {
                NamedDropdownItem { name: key.name(), item: *key }
            }).collect::<Vec<_>>();

            let row = items.iter().position(|v| v.item == sort_key).unwrap_or(0);
            SelectState::new(NamedDropdown::new(items), Some(IndexPath::new(row)), window, cx)
        });

        let content_list = cx.new(move |cx| {
            ListState::new(content_list_delegate, window, cx).selectable(false).searchable(true)
        });

        cx.observe(&content, |page, content, cx| {
            if let Some(new_content) = content.read(cx) {
                let (needs_update_check, update_count) = calculate_update_count(page.instance_loader, page.instance_version, new_content);
                page.needs_update_check = needs_update_check;
                page.update_count = update_count;
            }

            page.content_list.update(cx, |list, cx| {
                if let Some(new_content) = content.read(cx) {
                    list.delegate_mut().set_content(new_content);
                }
                cx.notify();
            });

            cx.notify();
        }).detach();

        cx.subscribe(&sort_dropdown, |this, _, event: &SelectEvent<NamedDropdown<InstanceContentSortKey>>, cx| {
            let SelectEvent::Confirm(Some(sort_key)) = event else {
                return;
            };

            let sort_key = *sort_key;
            let config = InterfaceConfig::get_mut(cx);

            if this.content_type.sort_key(config) == sort_key {
                return;
            }

            let enabled_first = this.content_type.sort_enabled_first(config);
            this.content_type.set_sort_key(config, sort_key);

            let content = this.content.read(cx).clone();
            let content_list = this.content_list.clone();
            cx.update_entity(&content_list, |list, cx| {
                list.delegate_mut().set_sort_options(sort_key, enabled_first);
                if let Some(content) = &content {
                    list.delegate_mut().set_content(content);
                }
                cx.notify();
            });
            cx.notify();
        }).detach();

        Self {
            content_type,
            instance: instance_id,
            instance_loader,
            instance_version,
            instance_name,
            backend_handle,
            content_states,
            content_list,
            updating,
            content,
            sort_dropdown,
            needs_update_check,
            update_count,
            _add_from_file_task: None,
        }
    }

    fn install_paths(&self, paths: &[PathBuf], window: &mut Window, cx: &mut App) {
        let content_folder = self.content_type.content_folder().folder_name();

        let content_install = ContentInstall {
            target: InstallTarget::Instance(self.instance),
            loader: self.instance_loader,
            minecraft_version: self.instance_version,
            files: paths.into_iter().filter_map(|path| {
                Some(ContentInstallFile {
                    replace_old: None,
                    path: bridge::install::ContentInstallPath::Raw(Path::new(content_folder).join(path.file_name()?).into()),
                    download: ContentDownload::File { path: path.clone() },
                    content_source: ContentSource::Manual,
                    reason: ContentInstallReason::Standalone,
                })
            }).collect(),
        };
        crate::root::start_install(content_install, &self.backend_handle, window, cx);
    }
}

fn calculate_update_count(loader: Loader, version: Ustr, content: &Arc<[InstanceContentSummary]>) -> (bool, usize) {
    let mut update_count = 0;

    for content in content.iter() {
        let status = content.update.status_if_matches(loader, version.as_str());
        match status {
            bridge::instance::ContentUpdateStatus::Unknown => {
                return (true, 0);
            },
            bridge::instance::ContentUpdateStatus::Modrinth | bridge::instance::ContentUpdateStatus::Curseforge => {
                update_count += 1;
            },
            _ => {},
        }
    }

    (false, update_count)
}

impl Render for InstanceContentSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let (source, sort_enabled_first) = {
            let config = InterfaceConfig::get(cx);
            (config.preferred_add_content_source, self.content_type.sort_enabled_first(config))
        };

        self.content_states.observe(self.content_type.content_folder());

        let self_entity = cx.entity();
        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child(self.content_type.title()))
            .child(Button::new("update_check").label(t::instance::content::update::check::label(false)).success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance_id = self.instance;
                move |_, window, cx| {
                    crate::root::start_update_check(instance_id, &backend_handle, window, cx);
                }
            }))
            .child(DropdownButton::new("addcontent")
                .success()
                .small()
                .button(match source {
                    PreferredAddContentSource::Modrinth => {
                        Button::new("addmr")
                            .compact()
                            .label(t::instance::content::install::from_modrinth())
                            .on_click(cx.listener(InstanceContentSubpage::add_from_modrinth))
                    },
                    PreferredAddContentSource::CurseForge => {
                        Button::new("addcf")
                            .compact()
                            .label(t::instance::content::install::from_curseforge())
                            .on_click(cx.listener(InstanceContentSubpage::add_from_curseforge))
                    },
                    PreferredAddContentSource::File => {
                        Button::new("addfile")
                            .compact()
                            .label(t::instance::content::install::from_file())
                            .on_click(cx.listener(InstanceContentSubpage::add_from_file))
                    },
                })
                .dropdown_menu(move |this, window, _| {
                    let mr = PopupMenuItem::new(t::instance::content::install::from_modrinth())
                            .on_click(window.listener_for(&self_entity, InstanceContentSubpage::add_from_modrinth));
                    let cf = PopupMenuItem::new(t::instance::content::install::from_curseforge())
                            .on_click(window.listener_for(&self_entity, InstanceContentSubpage::add_from_curseforge));
                    let file = PopupMenuItem::new(t::instance::content::install::from_file())
                            .on_click(window.listener_for(&self_entity, InstanceContentSubpage::add_from_file));

                    this.item(mr).item(cf).item(file)
                }))
            .when(!self.needs_update_check && self.update_count > 0, |this| {
                this.child(Button::new("update_all")
                    .label(match self.content_type {
                        ContentType::Mods => t::instance::content::update_all_mods(self.update_count),
                        ContentType::ResourcePacks => t::instance::content::update_all_resourcepacks(self.update_count),
                        ContentType::Shaders => t::instance::content::update_all_shaders(self.update_count),
                    })
                    .success()
                    .compact()
                    .small()
                    .on_click({
                        cx.listener(move |page, _, window, cx| {
                            if let Some(content) = page.content.read(cx).clone() {
                                let hashes: Vec<u64> = content.iter()
                                    .filter(|summary| summary.update.can_update(page.instance_loader, page.instance_version.as_str()))
                                    .map(|summary| summary.filename_hash)
                                    .collect();

                                page.updating.lock().extend(hashes.iter());
                                page.content_list.update(cx, |_, cx| cx.notify());

                                for summary in content.iter() {
                                    if summary.update.can_update(page.instance_loader, page.instance_version.as_str()) {
                                        crate::root::update_single_mod(page.instance, summary.id, &page.backend_handle, window, cx);
                                    }
                                }
                            }
                        })
                    }))
            });

        let filter_bar_controls = h_flex()
            .cursor_default()
            .block_mouse_except_scroll()
            .gap_3()
            .items_center()
            .child(div().child(Select::new(&self.sort_dropdown).small().title_prefix(t::instance::content::sort_prefix())))
            .child(h_flex().gap_1()
                .child(div().text_sm().child(t::instance::content::enabled_first()))
                .child(Switch::new("enabled_first")
                    .checked(sort_enabled_first)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        let config = InterfaceConfig::get_mut(cx);
                        let enabled_first = *checked;

                        if this.content_type.sort_enabled_first(config) == enabled_first {
                            return;
                        }

                        let sort_key = this.content_type.sort_key(config);
                        this.content_type.set_sort_enabled_first(config, enabled_first);

                        let content = this.content.read(cx).clone();
                        let content_list = this.content_list.clone();
                        cx.update_entity(&content_list, |list, cx| {
                            list.delegate_mut().set_sort_options(sort_key, enabled_first);
                            if let Some(content) = &content {
                                list.delegate_mut().set_content(content);
                            }
                            cx.notify();
                        });
                        cx.notify();
                    }))
                )
            )
            .absolute()
            .top(px(4.0))
            .right(px(12.0));

        let theme = cx.theme();
        v_flex().p_4().size_full()
            .child(header)
            .child(div()
                .id("content-list-area")
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
                .child(List::new(&self.content_list).search_placeholder(t::common::search()))
                .child(filter_bar_controls)
                .on_click({
                    let content_list = self.content_list.clone();
                    move |_, _, cx| {
                        cx.update_entity(&content_list, |list, cx| {
                            list.delegate_mut().clear_selection();
                            cx.notify();
                        })
                    }
                })
                .key_context("Input")
                .on_action({
                    let content_list = self.content_list.clone();
                    move |_: &SelectAll, _, cx| {
                        cx.update_entity(&content_list, |list, cx| {
                            list.delegate_mut().select_all();
                            cx.notify();
                        })
                    }
                }),
        )
    }

}

impl InstanceContentSubpage {
    fn add_from_modrinth(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<'_, InstanceContentSubpage>) {
        let config = InterfaceConfig::get_mut(cx);
        config.modrinth_page_project_type = self.content_type.modrinth_project_type();
        config.preferred_add_content_source = PreferredAddContentSource::Modrinth;

        let path = &[PageType::Instances, PageType::InstancePage { name: self.instance_name.clone() }];
        let page = crate::ui::PageType::Modrinth { installing_for: Some(self.instance_name.clone()) };
        root::switch_page(page, path, window, cx);
    }

    fn add_from_curseforge(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<'_, InstanceContentSubpage>) {
        let config = InterfaceConfig::get_mut(cx);
        config.curseforge_page_class_id = self.content_type.curseforge_class_id();
        config.preferred_add_content_source = PreferredAddContentSource::CurseForge;

        let path = &[PageType::Instances, PageType::InstancePage { name: self.instance_name.clone() }];
        let page = crate::ui::PageType::Curseforge { installing_for: Some(self.instance_name.clone()) };
        root::switch_page(page, path, window, cx);
    }

    fn add_from_file(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<'_, InstanceContentSubpage>) {
        let config = InterfaceConfig::get_mut(cx);
        config.preferred_add_content_source = PreferredAddContentSource::File;

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(self.content_type.install_select().into())
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
        self._add_from_file_task = Some(add_from_file_task);
    }
}
