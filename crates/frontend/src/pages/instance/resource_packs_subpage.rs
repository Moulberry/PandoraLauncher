use std::{hash::{DefaultHasher, Hash, Hasher}, path::{Path, PathBuf}, sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering}, Arc
}};

use bridge::{
    handle::BackendHandle, install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::{AtomicContentUpdateStatus, InstanceID, InstanceContentID, InstanceContentSummary, ContentType, ContentSummary}, message::{AtomicBridgeDataLoadState, MessageToBackend}, serial::AtomicOptionSerial
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Sizable, WindowExt, breadcrumb::{Breadcrumb, BreadcrumbItem}, button::{Button, ButtonVariants}, h_flex, input::SelectAll, list::{ListDelegate, ListItem, ListState}, notification::{Notification, NotificationType}, switch::Switch, v_flex
};
use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use schema::{content::ContentSource, loader::Loader, modrinth::ModrinthProjectType};
use ustr::Ustr;

use crate::{component::content_list::ContentListDelegate, entity::instance::InstanceEntry, interface_config::InterfaceConfig, png_render_cache, root, ui::PageType};

use super::instance_page::InstanceSubpageType;

pub struct InstanceResourcePacksSubpage {
    instance: InstanceID,
    instance_title: SharedString,
    instance_loader: Loader,
    instance_version: Ustr,
    backend_handle: BackendHandle,
    resource_packs_state: Arc<AtomicBridgeDataLoadState>,
    resource_pack_list: Entity<ListState<ContentListDelegate>>,
    load_serial: AtomicOptionSerial,
    _add_from_file_task: Option<Task<()>>,
}

impl InstanceResourcePacksSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let instance = instance.read(cx);
        let instance_title = instance.title().into();
        let instance_loader = instance.configuration.loader;
        let instance_version = instance.configuration.minecraft_version;
        let instance_id = instance.id;

        let resource_packs_state = Arc::clone(&instance.resource_packs_state);

        let mut resource_packs_list_delegate = ContentListDelegate::new(instance_id, backend_handle.clone());
        resource_packs_list_delegate.set_content(instance.resource_packs.read(cx));

        let resource_packs = instance.resource_packs.clone();

        let resource_pack_list = cx.new(move |cx| {
            cx.observe(&resource_packs, |list: &mut ListState<ContentListDelegate>, resource_packs, cx| {
                let actual_resource_packs = resource_packs.read(cx);
                list.delegate_mut().set_content(actual_resource_packs);
                cx.notify();
            }).detach();

            ListState::new(resource_packs_list_delegate, window, cx).selectable(false).searchable(true)
        });

        Self {
            instance: instance_id,
            instance_title,
            instance_loader,
            instance_version,
            backend_handle,
            resource_packs_state,
            resource_pack_list,
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
                    path: bridge::install::ContentInstallPath::Raw(Path::new("resourcepacks").join(path.file_name()?).into()),
                    download: ContentDownload::File { path: path.clone() },
                    content_source: ContentSource::Manual,
                })
            }).collect(),
        };
        crate::root::start_install(content_install, &self.backend_handle, window, cx);
    }
}

impl Render for InstanceResourcePacksSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        let state = self.resource_packs_state.load(Ordering::SeqCst);
        if state.should_send_load_request() {
            self.backend_handle.send_with_serial(MessageToBackend::RequestLoadResourcePacks { id: self.instance }, &self.load_serial);
        }

        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child("Resource Packs"))
            .child(Button::new("update").label("Check for updates").success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance_id = self.instance;
                move |_, window, cx| {
                    crate::root::start_update_check(instance_id, &backend_handle, window, cx);
                }
            }))
            .child(Button::new("addmr").label("Add from Modrinth").success().compact().small().on_click({
                let instance = self.instance;
                move |_, window, cx| {
                    let page = crate::ui::PageType::Modrinth {
                        installing_for: Some(instance),
                        project_type: Some(ModrinthProjectType::Resourcepack)
                    };
                    let path = &[PageType::Instances, PageType::InstancePage(instance, InstanceSubpageType::ResourcePacks)];
                    root::switch_page(page, path, window, cx);
                }
            }))
            .child(Button::new("addfile").label("Add from file").success().compact().small().on_click({
                cx.listener(move |this, _, window, cx| {
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: true,
                        prompt: Some("Select resource packs to install".into())
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
                .id("pack-list-area")
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
                .child(self.resource_pack_list.clone())
                .on_click({
                    let resource_pack_list = self.resource_pack_list.clone();
                    move |_, _, cx| {
                        cx.update_entity(&resource_pack_list, |list, _| {
                            list.delegate_mut().clear_selection();
                        })
                    }
                })
                .key_context("Input")
                .on_action({
                    let resource_pack_list = self.resource_pack_list.clone();
                    move |_: &SelectAll, _, cx| {
                        cx.update_entity(&resource_pack_list, |list, cx| {
                            list.delegate_mut().select_all();
                            cx.notify();
                        })
                    }
                }),
        )
    }
}
