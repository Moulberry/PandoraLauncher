use std::path::{Path, PathBuf};

use bridge::{
    handle::BackendHandle, install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::InstanceID, message::{BridgeDataLoadState, MessageToBackend}, serial::AtomicOptionSerial
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Sizable, WindowExt, button::{Button, ButtonVariants}, h_flex, input::SelectAll, list::ListState, notification::{Notification, NotificationType}, v_flex
};
use schema::{content::ContentSource, curseforge::CurseforgeClassId, loader::Loader, modrinth::ModrinthProjectType};
use ustr::Ustr;

use crate::{component::content_list::ContentListDelegate, entity::instance::InstanceEntry, interface_config::InterfaceConfig, root, ts, ui::PageType};

pub struct InstanceModsSubpage {
    instance: InstanceID,
    instance_loader: Loader,
    instance_version: Ustr,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    mods_state: BridgeDataLoadState,
    mod_list: Entity<ListState<ContentListDelegate>>,
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

        let mut mods_list_delegate = ContentListDelegate::new(instance_id, backend_handle.clone(), instance_loader, instance_version);
        mods_list_delegate.set_content(instance.mods.read(cx));

        let mods = instance.mods.clone();

        let mod_list = cx.new(move |cx| {
            cx.observe(&mods, |list: &mut ListState<ContentListDelegate>, mods, cx| {
                let actual_mods = mods.read(cx);
                list.delegate_mut().set_content(actual_mods);
                cx.notify();
            }).detach();

            ListState::new(mods_list_delegate, window, cx).selectable(false).searchable(true)
        });

        Self {
            instance: instance_id,
            instance_loader,
            instance_version,
            instance_name,
            backend_handle,
            mods_state,
            mod_list,
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
            .child(div().text_lg().child(ts!("instance.content.mods")))
            .child(Button::new("update").label(ts!("instance.content.update.check.label")).success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance_id = self.instance;
                move |_, window, cx| {
                    crate::root::start_update_check(instance_id, &backend_handle, window, cx);
                }
            }))
            .child(Button::new("addmr").label(ts!("instance.content.install.from_modrinth")).success().compact().small().on_click({
                let instance_name = self.instance_name.clone();
                move |_, window, cx| {
                    let page = crate::ui::PageType::Modrinth { installing_for: Some(instance_name.clone()) };
                    InterfaceConfig::get_mut(cx).modrinth_page_project_type = ModrinthProjectType::Mod;
                    let path = &[PageType::Instances, PageType::InstancePage { name: instance_name.clone() }];
                    root::switch_page(page, path, window, cx);
                }
            }))
            .child(Button::new("addcf").label(ts!("instance.content.install.from_curseforge")).success().compact().small().on_click({
                let instance_name = self.instance_name.clone();
                move |_, window, cx| {
                    let page = crate::ui::PageType::Curseforge { installing_for: Some(instance_name.clone()) };
                    InterfaceConfig::get_mut(cx).curseforge_page_class_id = CurseforgeClassId::Mod;
                    let path = &[PageType::Instances, PageType::InstancePage { name: instance_name.clone() }];
                    root::switch_page(page, path, window, cx);
                }
            }))
            .child(Button::new("addfile").label(ts!("instance.content.install.from_file")).success().compact().small().on_click({
                cx.listener(move |this, _, window, cx| {
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: true,
                        prompt: Some(ts!("instance.content.install.select_mods"))
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

        v_flex().p_4().size_full()
            .child(header)
            .child(div()
                .id("mod-list-area")
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
