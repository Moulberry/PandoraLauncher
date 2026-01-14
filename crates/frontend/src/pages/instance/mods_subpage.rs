use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use bridge::{
    handle::BackendHandle,
    install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget},
    instance::{
        AtomicContentUpdateStatus, InstanceID, InstanceModID, InstanceModSummary, LoaderSpecificModSummary, ModSummary,
    },
    message::{AtomicBridgeDataLoadState, MessageToBackend},
    serial::AtomicOptionSerial,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Sizable, WindowExt,
    breadcrumb::{Breadcrumb, BreadcrumbItem},
    button::{Button, ButtonVariants},
    h_flex,
    list::{ListDelegate, ListItem, ListState},
    notification::{Notification, NotificationType},
    switch::Switch,
    v_flex,
};
use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use schema::{content::ContentSource, loader::Loader};
use ustr::Ustr;

use crate::{entity::instance::InstanceEntry, png_render_cache, root};

use super::instance_page::InstanceSubpageType;

pub struct InstanceModsSubpage {
    instance: InstanceID,
    instance_title: SharedString,
    instance_loader: Loader,
    instance_version: Ustr,
    backend_handle: BackendHandle,
    mods_state: Arc<AtomicBridgeDataLoadState>,
    mod_list: Entity<ListState<ModsListDelegate>>,
    mods_serial: AtomicOptionSerial,
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
        let instance_title = instance.title().into();
        let instance_loader = instance.configuration.loader;
        let instance_version = instance.configuration.minecraft_version;
        let instance_id = instance.id;

        let mods_state = Arc::clone(&instance.mods_state);

        let mut mods_list_delegate = ModsListDelegate {
            id: instance_id,
            backend_handle: backend_handle.clone(),
            mods: Vec::new(),
            searched: None,
            children: Vec::new(),
            expanded: Arc::new(AtomicUsize::new(0)),
            confirming_delete: Default::default(),
            updating: Default::default(),
            last_query: SharedString::new_static(""),
            selected: FxHashSet::default(),
            selected_range: FxHashSet::default(),
            last_clicked_non_range: None,
        };
        mods_list_delegate.set_mods(instance.mods.read(cx));

        let mods = instance.mods.clone();

        let mod_list = cx.new(move |cx| {
            cx.observe(&mods, |list: &mut ListState<ModsListDelegate>, mods, cx| {
                let actual_mods = mods.read(cx);
                list.delegate_mut().set_mods(actual_mods);
                cx.notify();
            })
            .detach();

            ListState::new(mods_list_delegate, window, cx).selectable(false).searchable(true)
        });

        Self {
            instance: instance_id,
            instance_title,
            instance_loader,
            instance_version,
            backend_handle,
            mods_state,
            mod_list,
            mods_serial: AtomicOptionSerial::default(),
            _add_from_file_task: None,
        }
    }
}

impl Render for InstanceModsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        let state = self.mods_state.load(Ordering::SeqCst);
        if state.should_send_load_request() {
            self.backend_handle
                .send_with_serial(MessageToBackend::RequestLoadMods { id: self.instance }, &self.mods_serial);
        }

        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child("Mods"))
            .child(Button::new("update").label("Check for updates").success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance_id = self.instance;
                move |_, window, cx| {
                    crate::root::start_update_check(instance_id, &backend_handle, window, cx);
                }
            }))
            .child(Button::new("addmr").label("Add from Modrinth").success().compact().small().on_click({
                let instance = self.instance;
                let instance_title = self.instance_title.clone();
                move |_, window, cx| {
                    let page = crate::ui::PageType::Modrinth {
                        installing_for: Some(instance),
                    };

                    let instance_title = instance_title.clone();
                    let breadcrumb = move || {
                        let instances_item = BreadcrumbItem::new("Instances").on_click(|_, window, cx| {
                            root::switch_page(crate::ui::PageType::Instances, None, window, cx);
                        });
                        let instance_item =
                            BreadcrumbItem::new(instance_title.clone()).on_click(move |_, window, cx| {
                                root::switch_page(
                                    crate::ui::PageType::InstancePage(instance, InstanceSubpageType::Mods),
                                    None,
                                    window,
                                    cx,
                                );
                            });
                        Breadcrumb::new().text_xl().child(instances_item).child(instance_item)
                    };

                    root::switch_page(page, Some(Box::new(breadcrumb)), window, cx);
                }
            }))
            .child(Button::new("addfile").label("Add from file").success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance = self.instance;
                cx.listener(move |this, _, window, cx| {
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: true,
                        prompt: Some("Select mods to install".into()),
                    });

                    let backend_handle = backend_handle.clone();
                    let entity = cx.entity();
                    let add_from_file_task = window.spawn(cx, async move |cx| {
                        let Ok(result) = receiver.await else {
                            return;
                        };
                        _ = cx.update_window_entity(&entity, move |this, window, cx| match result {
                            Ok(Some(paths)) => {
                                let content_install = ContentInstall {
                                    target: InstallTarget::Instance(instance),
                                    loader_hint: this.instance_loader,
                                    version_hint: Some(this.instance_version.into()),
                                    files: paths
                                        .into_iter()
                                        .filter_map(|path| {
                                            Some(ContentInstallFile {
                                                replace_old: None,
                                                path: bridge::install::ContentInstallPath::Raw(
                                                    Path::new("mods").join(path.file_name()?).into(),
                                                ),
                                                download: ContentDownload::File { path },
                                                content_source: ContentSource::Manual,
                                            })
                                        })
                                        .collect(),
                                };
                                crate::root::start_install(content_install, &backend_handle, window, cx);
                            },
                            Ok(None) => {},
                            Err(error) => {
                                let error = format!("{}", error);
                                let notification =
                                    Notification::new().autohide(false).with_type(NotificationType::Error).title(error);
                                window.push_notification(notification, cx);
                            },
                        });
                    });
                    this._add_from_file_task = Some(add_from_file_task);
                })
            }));

        let mod_list = self.mod_list.clone();
        v_flex().p_4().size_full().child(header).child(
            div()
                .id("mod-list-area")
                .size_full()
                .border_1()
                .rounded(theme.radius)
                .border_color(theme.border)
                .child(self.mod_list.clone())
                .on_click(move |_, _, cx| {
                    cx.update_entity(&mod_list, |list, _| {
                        let delegate = list.delegate_mut();
                        delegate.selected.clear();
                        delegate.selected_range.clear();
                        delegate.last_clicked_non_range = None;
                        delegate.confirming_delete.lock().clear();
                    })
                }),
        )
    }
}

#[derive(Clone)]
struct ModEntryChild {
    summary: Arc<ModSummary>,
    parent: InstanceModID,
    path: Arc<str>,
    lowercase_filename: Arc<str>,
    enabled: bool,
    parent_enabled: bool,
}

enum InstanceModSummaryOrChild {
    InstanceModSummary(InstanceModSummary),
    ModEntryChild(ModEntryChild),
}

pub struct ModsListDelegate {
    id: InstanceID,
    backend_handle: BackendHandle,
    mods: Vec<InstanceModSummary>,
    searched: Option<Vec<InstanceModSummaryOrChild>>,
    children: Vec<Vec<ModEntryChild>>,
    expanded: Arc<AtomicUsize>,
    confirming_delete: Arc<Mutex<FxHashSet<u64>>>,
    updating: Arc<Mutex<FxHashSet<u64>>>,
    last_query: SharedString,
    selected: FxHashSet<u64>,
    selected_range: FxHashSet<u64>,
    last_clicked_non_range: Option<u64>,
}

impl ModsListDelegate {
    pub fn render_instance_mod_summary(
        &self,
        summary: &InstanceModSummary,
        selected: bool,
        expanded: bool,
        can_expand: bool,
        ix: usize,
        cx: &mut Context<ListState<Self>>,
    ) -> ListItem {
        let icon = if let Some(png_icon) = summary.mod_summary.png_icon.as_ref() {
            png_render_cache::render(Arc::clone(png_icon), cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded("images/default_mod.png".into())))
        };

        const GRAY: Hsla = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.5,
            a: 1.0,
        };

        let description1 = v_flex()
            .w_1_5()
            .text_ellipsis()
            .child(SharedString::from(summary.mod_summary.name.clone()))
            .child(SharedString::from(summary.mod_summary.version_str.clone()));

        let description2 = v_flex()
            .text_color(GRAY)
            .child(SharedString::from(summary.mod_summary.authors.clone()))
            .child(SharedString::from(summary.filename.clone()));

        let id = self.id;
        let mod_id = summary.id;
        let element_id = summary.filename_hash;

        let delete_button = if self.confirming_delete.lock().contains(&element_id) {
            Button::new(("delete", element_id)).danger().icon(IconName::Check).on_click({
                let backend_handle = self.backend_handle.clone();
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let delegate = this.delegate();
                    if delegate.is_selected(element_id) {
                        let mod_ids = delegate
                            .mods
                            .iter()
                            .filter_map(|summary| {
                                if delegate.is_selected(summary.filename_hash) {
                                    Some(summary.id)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        backend_handle.send(MessageToBackend::DeleteMod { id, mod_ids });
                    } else {
                        backend_handle.send(MessageToBackend::DeleteMod {
                            id,
                            mod_ids: vec![mod_id],
                        });
                    }
                })
            })
        } else {
            let trash_icon = Icon::default().path("icons/trash-2.svg");
            let confirming_delete = self.confirming_delete.clone();
            Button::new(("delete", element_id)).danger().icon(trash_icon).on_click(cx.listener(
                move |this, _checked, _, cx| {
                    cx.stop_propagation();
                    let delegate = this.delegate();
                    let mut confirming_delete = confirming_delete.lock();
                    confirming_delete.clear();
                    if delegate.is_selected(element_id) {
                        confirming_delete.extend(&delegate.selected);
                        confirming_delete.extend(&delegate.selected_range);
                    } else {
                        confirming_delete.insert(element_id);
                    }
                },
            ))
        };

        let update_button = match summary.mod_summary.update_status.load(Ordering::Relaxed) {
            bridge::instance::ContentUpdateStatus::Unknown => None,
            bridge::instance::ContentUpdateStatus::ManualInstall => Some(
                Button::new(("update", element_id))
                    .warning()
                    .icon(Icon::default().path("icons/file-question-mark.svg"))
                    .tooltip("Mod was installed manually - cannot automatically update"),
            ),
            bridge::instance::ContentUpdateStatus::ErrorNotFound => Some(
                Button::new(("update", element_id))
                    .danger()
                    .icon(Icon::default().path("icons/triangle-alert.svg"))
                    .tooltip("Error while checking updates - 404 not found"),
            ),
            bridge::instance::ContentUpdateStatus::ErrorInvalidHash => Some(
                Button::new(("update", element_id))
                    .danger()
                    .icon(Icon::default().path("icons/triangle-alert.svg"))
                    .tooltip("Error while checking updates - returned invalid hash"),
            ),
            bridge::instance::ContentUpdateStatus::AlreadyUpToDate => Some(
                Button::new(("update", element_id))
                    .icon(Icon::default().path("icons/check.svg"))
                    .tooltip("Mod is up-to-date as of last check"),
            ),
            bridge::instance::ContentUpdateStatus::Modrinth => {
                let loading = self.updating.lock().contains(&element_id);
                Some(
                    Button::new(("update", element_id))
                        .success()
                        .loading(loading)
                        .icon(Icon::default().path("icons/download.svg"))
                        .tooltip("Download update from Modrinth")
                        .on_click({
                            let backend_handle = self.backend_handle.clone();
                            let updating = self.updating.clone();
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();

                                let mut updating = updating.lock();
                                let delegate = this.delegate_mut();
                                if delegate.is_selected(element_id) {
                                    for summary in &delegate.mods {
                                        if delegate.is_selected(summary.filename_hash)
                                            && summary.mod_summary.update_status.load(Ordering::Relaxed).can_update()
                                        {
                                            updating.insert(summary.filename_hash);
                                            crate::root::update_single_mod(id, summary.id, &backend_handle, window, cx);
                                        }
                                    }
                                    delegate.selected.clear();
                                    delegate.selected_range.clear();
                                    delegate.last_clicked_non_range = None;
                                } else {
                                    updating.insert(element_id);
                                    crate::root::update_single_mod(id, mod_id, &backend_handle, window, cx);
                                }
                            })
                        }),
                )
            },
        };

        let backend_handle = self.backend_handle.clone();

        let toggle_control = Switch::new(("toggle", element_id))
            .checked(summary.enabled)
            .on_click(cx.listener(move |this, checked, _, _| {
                let delegate = this.delegate();
                if delegate.is_selected(element_id) {
                    let mod_ids = delegate
                        .mods
                        .iter()
                        .filter_map(|summary| {
                            if delegate.is_selected(summary.filename_hash) {
                                Some(summary.id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    backend_handle.send(MessageToBackend::SetModEnabled {
                        id,
                        mod_ids,
                        enabled: *checked,
                    });
                } else {
                    backend_handle.send(MessageToBackend::SetModEnabled {
                        id,
                        mod_ids: vec![mod_id],
                        enabled: *checked,
                    });
                }
            }))
            .px_2();

        let controls = if !can_expand {
            toggle_control.into_any_element()
        } else {
            let expand_icon = if expanded {
                IconName::ArrowDown
            } else {
                IconName::ArrowRight
            };

            let expand_control =
                Button::new(("expand", element_id)).icon(expand_icon).compact().small().info().on_click({
                    let expanded = self.expanded.clone();
                    let index = ix + 1;
                    move |_, _, _| {
                        let value = expanded.load(Ordering::Relaxed);
                        if value == index {
                            expanded.store(0, Ordering::Relaxed);
                        } else {
                            expanded.store(index, Ordering::Relaxed);
                        }
                    }
                });

            v_flex()
                .items_center()
                .gap_1()
                .child(toggle_control)
                .child(expand_control)
                .into_any_element()
        };

        let mut item_content = h_flex()
            .gap_1()
            .child(controls)
            .child(icon.size_16().min_w_16().min_h_16().grayscale(!summary.enabled))
            .when(!summary.enabled, |this| this.line_through())
            .child(description1)
            .child(description2)
            .border_1()
            .when(selected, |content| {
                content.border_color(cx.theme().selection).bg(cx.theme().selection.alpha(0.2))
            });

        if let Some(update_button) = update_button {
            item_content =
                item_content.child(h_flex().absolute().right_4().gap_2().child(update_button).child(delete_button))
        } else {
            item_content = item_content.child(delete_button.absolute().right_4())
        }

        ListItem::new(("item", element_id)).p_1().child(item_content).on_click(cx.listener(
            move |this, click: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if click.standard_click() {
                    let delegate = this.delegate_mut();
                    delegate.confirming_delete.lock().clear();
                    if click.modifiers().shift
                        && let Some(from) = delegate.last_clicked_non_range
                    {
                        delegate.selected_range.clear();

                        if let Some(searched) = &delegate.searched {
                            let from_index = searched.iter().position(|element| match element {
                                InstanceModSummaryOrChild::InstanceModSummary(instance_mod_summary) => {
                                    instance_mod_summary.filename_hash == from
                                },
                                InstanceModSummaryOrChild::ModEntryChild(_) => false,
                            });

                            let Some(from_index) = from_index else {
                                return;
                            };

                            let to_index = searched.iter().position(|element| match element {
                                InstanceModSummaryOrChild::InstanceModSummary(instance_mod_summary) => {
                                    instance_mod_summary.filename_hash == element_id
                                },
                                InstanceModSummaryOrChild::ModEntryChild(_) => false,
                            });

                            let Some(to_index) = to_index else {
                                return;
                            };

                            let min_index = from_index.min(to_index);
                            let max_index = from_index.max(to_index);

                            for add in searched[min_index..=max_index].iter() {
                                match add {
                                    InstanceModSummaryOrChild::InstanceModSummary(instance_mod_summary) => {
                                        delegate.selected_range.insert(instance_mod_summary.filename_hash);
                                    },
                                    InstanceModSummaryOrChild::ModEntryChild(_) => {},
                                }
                            }
                        } else {
                            let from_index = delegate.mods.iter().position(|element| element.filename_hash == from);

                            let Some(from_index) = from_index else {
                                return;
                            };

                            let to_index = delegate.mods.iter().position(|element| element.filename_hash == element_id);

                            let Some(to_index) = to_index else {
                                return;
                            };

                            let min_index = from_index.min(to_index);
                            let max_index = from_index.max(to_index);

                            for add in delegate.mods[min_index..=max_index].iter() {
                                delegate.selected_range.insert(add.filename_hash);
                            }
                        }
                    } else if click.modifiers().secondary() || click.modifiers().shift {
                        // Cmd+Click (macos), Ctrl+Click (win/linux)

                        delegate.selected.extend(&delegate.selected_range);
                        delegate.selected_range.clear();

                        if delegate.selected.contains(&element_id) {
                            delegate.selected.remove(&element_id);
                        } else {
                            delegate.selected.insert(element_id);
                        }

                        delegate.last_clicked_non_range = Some(element_id);
                    } else {
                        delegate.selected_range.clear();
                        delegate.selected.clear();
                        delegate.selected.insert(element_id);
                        delegate.last_clicked_non_range = Some(element_id);
                    }
                }
            },
        ))
    }

    fn render_child_entry(&self, child: &ModEntryChild, cx: &mut App) -> ListItem {
        let summary = &child.summary;
        let icon = if let Some(png_icon) = summary.png_icon.as_ref() {
            png_render_cache::render(Arc::clone(png_icon), cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded("images/default_mod.png".into())))
        };

        const GRAY: Hsla = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.5,
            a: 1.0,
        };

        let description1 = v_flex()
            .w_1_5()
            .text_ellipsis()
            .child(SharedString::from(summary.name.clone()))
            .child(SharedString::from(summary.version_str.clone()));

        let description2 = v_flex()
            .text_color(GRAY)
            .child(SharedString::from(summary.authors.clone()))
            .child(SharedString::from(child.path.clone()));

        let mut hasher = DefaultHasher::new();
        child.parent.hash(&mut hasher);
        child.path.hash(&mut hasher);
        let element_id = hasher.finish();

        let enabled = child.enabled;
        let visually_enabled = enabled && child.parent_enabled;

        let item_content = h_flex()
            .gap_1()
            .pl_4()
            .child(
                Switch::new(("toggle", element_id))
                    .checked(enabled)
                    .on_click({
                        let id = self.id;
                        let mod_id = child.parent;
                        let path = child.path.clone();
                        let backend_handle = self.backend_handle.clone();
                        move |checked, _, _| {
                            backend_handle.send(MessageToBackend::SetModChildEnabled {
                                id,
                                mod_id,
                                path: path.clone(),
                                enabled: *checked,
                            });
                        }
                    })
                    .px_2(),
            )
            .child(icon.size_16().min_w_16().min_h_16().grayscale(!visually_enabled))
            .when(!visually_enabled, |this| this.line_through())
            .child(description1)
            .child(description2);

        ListItem::new(("item", element_id)).p_1().child(item_content)
    }

    fn set_mods(&mut self, actual_mods: &[InstanceModSummary]) {
        let last_mods_len = self.mods.len();

        let mut mods = Vec::with_capacity(actual_mods.len());
        let mut children = Vec::with_capacity(actual_mods.len());

        let unknown = Arc::new(bridge::instance::ModSummary {
            id: "".into(),
            hash: [0_u8; 20],
            name: "Unknown".into(),
            lowercase_search_key: "unknown".into(),
            version_str: "unknown".into(),
            authors: "Unknown".into(),
            png_icon: None,
            update_status: Arc::new(AtomicContentUpdateStatus::new(bridge::instance::ContentUpdateStatus::Unknown)),
            extra: LoaderSpecificModSummary::Fabric,
        });

        for modification in actual_mods.iter() {
            mods.push(modification.clone());

            if let LoaderSpecificModSummary::ModrinthModpack {
                downloads, summaries, ..
            } = &modification.mod_summary.extra
            {
                let mut inner_children = Vec::new();
                for (index, download) in downloads.iter().enumerate() {
                    if !download.path.starts_with("mods/") {
                        continue;
                    }

                    let summary = summaries.get(index).cloned().flatten().unwrap_or(unknown.clone());

                    let enabled = !modification.disabled_children.contains(&*download.path);

                    let lowercase_filename = download.path.to_lowercase();

                    inner_children.push(ModEntryChild {
                        summary,
                        parent: modification.id,
                        lowercase_filename: lowercase_filename.into(),
                        path: download.path.clone(),
                        enabled,
                        parent_enabled: modification.enabled,
                    });
                }
                inner_children
                    .sort_by(|a, b| lexical_sort::natural_lexical_cmp(&a.lowercase_filename, &b.lowercase_filename));
                children.push(inner_children);
            } else {
                children.push(Vec::new());
            }
        }

        let mut updating = self.updating.lock();
        if !updating.is_empty() {
            let ids: FxHashSet<u64> = mods.iter().map(|summary| summary.filename_hash).collect();
            updating.retain(|id| ids.contains(&id));
        }
        drop(updating);

        self.mods = mods.clone();
        self.children = children;
        self.searched = None;
        self.confirming_delete.lock().clear();
        if last_mods_len != self.mods.len() {
            self.expanded.store(0, Ordering::Release);
        }
        let _ = self.actual_perform_search(&self.last_query.clone());
    }

    fn actual_perform_search(&mut self, query: &str) {
        let query = query.trim_ascii();

        self.last_clicked_non_range = None;

        if query.is_empty() {
            self.last_query = SharedString::new_static("");
            self.searched = None;
            return;
        }

        self.last_query = SharedString::new(query);

        let query = query.to_lowercase();

        let mut searched = Vec::new();

        for (m, children) in self.mods.iter().zip(self.children.iter()) {
            let mut parent_added = false;

            if m.mod_summary.lowercase_search_key.contains(&query) || m.lowercase_filename.contains(&query) {
                parent_added = true;
                searched.push(InstanceModSummaryOrChild::InstanceModSummary(m.clone()));
            }

            for child in children {
                if child.summary.lowercase_search_key.contains(&query) || child.lowercase_filename.contains(&query) {
                    if !parent_added {
                        parent_added = true;
                        searched.push(InstanceModSummaryOrChild::InstanceModSummary(m.clone()));
                    }

                    searched.push(InstanceModSummaryOrChild::ModEntryChild(child.clone()));
                }
            }
        }

        self.searched = Some(searched);
    }

    fn is_selected(&self, element_id: u64) -> bool {
        self.selected.contains(&element_id) || self.selected_range.contains(&element_id)
    }
}

impl ListDelegate for ModsListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        if let Some(searched) = &self.searched {
            return searched.len();
        }

        let expanded = self.expanded.load(Ordering::Relaxed);
        if expanded > 0 {
            self.mods.len() + self.children[expanded - 1].len()
        } else {
            self.mods.len()
        }
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let mut index = ix.row;

        if let Some(searched) = &self.searched {
            let item = searched.get(index)?;
            match item {
                InstanceModSummaryOrChild::InstanceModSummary(instance_mod_summary) => {
                    let selected = self.is_selected(instance_mod_summary.filename_hash);
                    return Some(self.render_instance_mod_summary(
                        instance_mod_summary,
                        selected,
                        false,
                        false,
                        ix.row,
                        cx,
                    ));
                },
                InstanceModSummaryOrChild::ModEntryChild(mod_entry_child) => {
                    return Some(self.render_child_entry(mod_entry_child, cx));
                },
            }
        }

        let expanded = self.expanded.load(Ordering::Relaxed);

        if expanded > 0 && index >= expanded {
            if let Some(child) = self.children[expanded - 1].get(index - expanded) {
                return Some(self.render_child_entry(child, cx));
            }
            index -= self.children[expanded - 1].len();
        }

        let summary = self.mods.get(index)?;
        let selected = self.is_selected(summary.filename_hash);
        Some(self.render_instance_mod_summary(
            summary,
            selected,
            index + 1 == expanded,
            !self.children[index].is_empty(),
            ix.row,
            cx,
        ))
    }

    fn set_selected_index(&mut self, _ix: Option<IndexPath>, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {
    }

    fn perform_search(&mut self, query: &str, _window: &mut Window, _cx: &mut Context<ListState<Self>>) -> Task<()> {
        self.actual_perform_search(query);
        Task::ready(())
    }
}
