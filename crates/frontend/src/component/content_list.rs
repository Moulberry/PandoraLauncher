use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use regex::Regex;

use bridge::{
    handle::BackendHandle,
    instance::{
        AtomicContentUpdateStatus, ContentSummary, ContentType, InstanceContentID, InstanceContentSummary, InstanceID,
    },
    message::MessageToBackend,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{ListDelegate, ListItem, ListState},
    switch::Switch,
    v_flex,
};
use parking_lot::Mutex;
use rustc_hash::FxHashSet;

use crate::{component::minecraft_colors::*, interface_config::InterfaceConfig, png_render_cache, ts};

#[derive(Clone)]
struct ContentEntryChild {
    summary: Arc<ContentSummary>,
    parent_filename_hash: u64,
    parent: InstanceContentID,
    path: Arc<str>,
    lowercase_search_keys: Arc<[Arc<str>]>,
    enabled: bool,
    parent_enabled: bool,
}

enum SummaryOrChild {
    Summary(InstanceContentSummary),
    Child(ContentEntryChild),
}

pub struct ContentListDelegate {
    id: InstanceID,
    backend_handle: BackendHandle,
    content: Vec<InstanceContentSummary>,
    searched: Option<Vec<SummaryOrChild>>,
    children: Vec<Vec<ContentEntryChild>>,
    expanded: Arc<AtomicUsize>,
    confirming_delete: Arc<Mutex<FxHashSet<u64>>>,
    updating: Arc<Mutex<FxHashSet<u64>>>,
    last_query: SharedString,
    selected: FxHashSet<u64>,
    selected_range: FxHashSet<u64>,
    last_clicked_non_range: Option<u64>,
}

impl ContentListDelegate {
    pub fn new(id: InstanceID, backend_handle: BackendHandle) -> Self {
        Self {
            id,
            backend_handle,
            content: Vec::new(),
            searched: None,
            children: Vec::new(),
            expanded: Arc::new(AtomicUsize::new(0)),
            confirming_delete: Default::default(),
            updating: Default::default(),
            last_query: SharedString::new_static(""),
            selected: FxHashSet::default(),
            selected_range: FxHashSet::default(),
            last_clicked_non_range: None,
        }
    }

    pub fn render_summary(
        &self,
        summary: &InstanceContentSummary,
        selected: bool,
        expanded: bool,
        can_expand: bool,
        ix: usize,
        cx: &mut Context<ListState<Self>>,
    ) -> ListItem {
        let icon = if let Some(png_icon) = summary.content_summary.png_icon.as_ref() {
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

        let (desc1, desc2) = create_descriptions(
            &summary.content_summary.name,
            &summary.content_summary.version_str,
            &summary.content_summary.authors,
            &summary.filename,
        );

        let id = self.id;
        let content_id = summary.id;
        let element_id = summary.filename_hash;

        let delete_button = if self.confirming_delete.lock().contains(&element_id) {
            Button::new(("delete", element_id)).danger().icon(IconName::Check).on_click({
                let backend_handle = self.backend_handle.clone();
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let delegate = this.delegate();
                    if delegate.is_selected(element_id) {
                        let content_ids = delegate
                            .content
                            .iter()
                            .filter_map(|summary| delegate.is_selected(summary.filename_hash).then(|| summary.id))
                            .collect();

                        backend_handle.send(MessageToBackend::DeleteContent { id, content_ids });
                    } else {
                        backend_handle.send(MessageToBackend::DeleteContent {
                            id,
                            content_ids: vec![content_id],
                        });
                    }
                })
            })
        } else {
            let trash_icon = Icon::default().path("icons/trash-2.svg");
            let confirming_delete = self.confirming_delete.clone();
            let backend_handle = self.backend_handle.clone();
            Button::new(("delete", element_id)).danger().icon(trash_icon).on_click(cx.listener(
                move |this, click: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    let delegate = this.delegate();

                    // If quick_delete_mods is enabled and shift clicking, delete instantly
                    if InterfaceConfig::get(cx).quick_delete_mods && click.modifiers().shift {
                        if delegate.is_selected(element_id) {
                            let content_ids = delegate
                                .content
                                .iter()
                                .filter_map(|summary| delegate.is_selected(summary.filename_hash).then(|| summary.id))
                                .collect();

                            backend_handle.send(MessageToBackend::DeleteContent { id, content_ids });
                        } else {
                            backend_handle.send(MessageToBackend::DeleteContent {
                                id,
                                content_ids: vec![content_id],
                            });
                        }
                        return;
                    }

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

        let update_button = match summary.content_summary.update_status.load(Ordering::Relaxed) {
            bridge::instance::ContentUpdateStatus::Unknown => None,
            bridge::instance::ContentUpdateStatus::ManualInstall => Some(
                Button::new(("update", element_id))
                    .warning()
                    .icon(Icon::default().path("icons/file-question-mark.svg"))
                    .tooltip(ts!("instance.content.update.installed_manually")),
            ),
            bridge::instance::ContentUpdateStatus::ErrorNotFound => Some(
                Button::new(("update", element_id))
                    .danger()
                    .icon(Icon::default().path("icons/triangle-alert.svg"))
                    .tooltip(ts!("instance.content.update.check.error_404")),
            ),
            bridge::instance::ContentUpdateStatus::ErrorInvalidHash => Some(
                Button::new(("update", element_id))
                    .danger()
                    .icon(Icon::default().path("icons/triangle-alert.svg"))
                    .tooltip(ts!("instance.content.update.check.invalid_hash_error")),
            ),
            bridge::instance::ContentUpdateStatus::AlreadyUpToDate => Some(
                Button::new(("update", element_id))
                    .icon(Icon::default().path("icons/check.svg"))
                    .tooltip(ts!("instance.content.update.check.last_up_to_date")),
            ),
            bridge::instance::ContentUpdateStatus::Modrinth => {
                let loading = self.updating.lock().contains(&element_id);
                Some(
                    Button::new(("update", element_id))
                        .success()
                        .loading(loading)
                        .icon(Icon::default().path("icons/download.svg"))
                        .tooltip(ts!("instance.content.update.download.from_modrinth"))
                        .on_click({
                            let backend_handle = self.backend_handle.clone();
                            let updating = self.updating.clone();
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();

                                let mut updating = updating.lock();
                                let delegate = this.delegate_mut();
                                if delegate.is_selected(element_id) {
                                    for summary in &delegate.content {
                                        if delegate.is_selected(summary.filename_hash)
                                            && summary
                                                .content_summary
                                                .update_status
                                                .load(Ordering::Relaxed)
                                                .can_update()
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
                                    crate::root::update_single_mod(id, content_id, &backend_handle, window, cx);
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
                    let content_ids = delegate
                        .content
                        .iter()
                        .filter_map(|summary| {
                            if delegate.is_selected(summary.filename_hash) {
                                Some(summary.id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    backend_handle.send(MessageToBackend::SetContentEnabled {
                        id,
                        content_ids,
                        enabled: *checked,
                    });
                } else {
                    backend_handle.send(MessageToBackend::SetContentEnabled {
                        id,
                        content_ids: vec![content_id],
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
            .child(desc1)
            .when_some(desc2, |div, desc2| div.child(desc2))
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
                                SummaryOrChild::Summary(summary) => summary.filename_hash == from,
                                SummaryOrChild::Child(_) => false,
                            });

                            let Some(from_index) = from_index else {
                                return;
                            };

                            let to_index = searched.iter().position(|element| match element {
                                SummaryOrChild::Summary(summary) => summary.filename_hash == element_id,
                                SummaryOrChild::Child(_) => false,
                            });

                            let Some(to_index) = to_index else {
                                return;
                            };

                            let min_index = from_index.min(to_index);
                            let max_index = from_index.max(to_index);

                            for add in searched[min_index..=max_index].iter() {
                                match add {
                                    SummaryOrChild::Summary(summary) => {
                                        delegate.selected_range.insert(summary.filename_hash);
                                    },
                                    SummaryOrChild::Child(_) => {},
                                }
                            }
                        } else {
                            let from_index = delegate.content.iter().position(|element| element.filename_hash == from);

                            let Some(from_index) = from_index else {
                                return;
                            };

                            let to_index =
                                delegate.content.iter().position(|element| element.filename_hash == element_id);

                            let Some(to_index) = to_index else {
                                return;
                            };

                            let min_index = from_index.min(to_index);
                            let max_index = from_index.max(to_index);

                            for add in delegate.content[min_index..=max_index].iter() {
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

    fn render_child_entry(&self, child: &ContentEntryChild, cx: &mut App) -> ListItem {
        let summary = &child.summary;
        let icon = if let Some(png_icon) = summary.png_icon.as_ref() {
            png_render_cache::render(Arc::clone(png_icon), cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded("images/default_mod.png".into())))
        };

        let (desc1, desc2) = create_descriptions(&summary.name, &summary.version_str, &summary.authors, &child.path);

        let mut hasher = DefaultHasher::new();
        child.parent_filename_hash.hash(&mut hasher);
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
                        let content_id = child.parent;
                        let child_id = child.summary.id.clone();
                        let child_name = child.summary.name.clone();
                        let path = child.path.clone();
                        let backend_handle = self.backend_handle.clone();
                        move |checked, _, _| {
                            backend_handle.send(MessageToBackend::SetContentChildEnabled {
                                id,
                                content_id,
                                child_id: child_id.clone(),
                                child_name: child_name.clone(),
                                child_filename: path.clone(),
                                enabled: *checked,
                            });
                        }
                    })
                    .px_2(),
            )
            .child(icon.size_16().min_w_16().min_h_16().grayscale(!visually_enabled))
            .when(!visually_enabled, |this| this.line_through())
            .child(desc1)
            .when_some(desc2, |div, desc2| div.child(desc2));

        ListItem::new(("item", element_id)).p_1().child(item_content)
    }

    pub fn set_content(&mut self, new_content: &[InstanceContentSummary]) {
        let last_mods_len = self.content.len();

        let mut mods = Vec::with_capacity(new_content.len());
        let mut children = Vec::with_capacity(new_content.len());

        let unknown = Arc::new(bridge::instance::ContentSummary {
            id: None,
            hash: [0_u8; 20],
            name: None,
            version_str: "unknown".into(),
            authors: "".into(),
            png_icon: None,
            update_status: Arc::new(AtomicContentUpdateStatus::new(bridge::instance::ContentUpdateStatus::Unknown)),
            extra: ContentType::Fabric,
        });

        for modification in new_content.iter() {
            mods.push(modification.clone());

            if let ContentType::ModrinthModpack {
                downloads, summaries, ..
            } = &modification.content_summary.extra
            {
                let mut inner_children = Vec::new();
                for (index, download) in downloads.iter().enumerate() {
                    if !download.path.starts_with("mods/") {
                        continue;
                    }

                    let summary = summaries.get(index).cloned().flatten().unwrap_or(unknown.clone());

                    let enabled = if let Some(id) = &summary.id
                        && modification.disabled_children.disabled_ids.contains(id)
                    {
                        false
                    } else if let Some(name) = &summary.name
                        && modification.disabled_children.disabled_names.contains(name)
                    {
                        false
                    } else {
                        !modification.disabled_children.disabled_filenames.contains(&*download.path)
                    };

                    let lowercase_filename: Arc<str> = download.path.to_lowercase().into();

                    let lowercase_search_keys = summary
                        .id
                        .clone()
                        .into_iter()
                        .chain(summary.name.clone().into_iter())
                        .chain(std::iter::once(lowercase_filename))
                        .collect();

                    inner_children.push(ContentEntryChild {
                        summary,
                        parent_filename_hash: modification.filename_hash,
                        parent: modification.id,
                        lowercase_search_keys,
                        path: download.path.clone(),
                        enabled,
                        parent_enabled: modification.enabled,
                    });
                }
                inner_children.sort_by(|a, b| {
                    lexical_sort::natural_lexical_cmp(
                        &a.lowercase_search_keys.last().unwrap(),
                        &b.lowercase_search_keys.last().unwrap(),
                    )
                });
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

        self.content = mods.clone();
        self.children = children;
        self.searched = None;
        self.confirming_delete.lock().clear();
        if last_mods_len != self.content.len() {
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

        for (m, children) in self.content.iter().zip(self.children.iter()) {
            let mut parent_added = false;

            if m.lowercase_search_keys.iter().any(|f| f.contains(&query)) {
                parent_added = true;
                searched.push(SummaryOrChild::Summary(m.clone()));
            }

            for child in children {
                if child.lowercase_search_keys.iter().any(|f| f.contains(&query)) {
                    if !parent_added {
                        parent_added = true;
                        searched.push(SummaryOrChild::Summary(m.clone()));
                    }

                    searched.push(SummaryOrChild::Child(child.clone()));
                }
            }
        }

        self.searched = Some(searched);
    }

    fn is_selected(&self, element_id: u64) -> bool {
        self.selected.contains(&element_id) || self.selected_range.contains(&element_id)
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.selected_range.clear();
        self.last_clicked_non_range = None;
        self.confirming_delete.lock().clear();
    }

    pub fn select_all(&mut self) {
        self.clear_selection();

        if let Some(searched) = &self.searched {
            for element in searched {
                if let SummaryOrChild::Summary(summary) = element {
                    self.selected.insert(summary.filename_hash);
                }
            }
        } else {
            for summary in &self.content {
                self.selected.insert(summary.filename_hash);
            }
        }
    }
}

impl ListDelegate for ContentListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        if let Some(searched) = &self.searched {
            return searched.len();
        }

        let expanded = self.expanded.load(Ordering::Relaxed);
        if expanded > 0 {
            self.content.len() + self.children[expanded - 1].len()
        } else {
            self.content.len()
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
                SummaryOrChild::Summary(instance_mod_summary) => {
                    let selected = self.is_selected(instance_mod_summary.filename_hash);
                    return Some(self.render_summary(instance_mod_summary, selected, false, false, ix.row, cx));
                },
                SummaryOrChild::Child(mod_entry_child) => {
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

        let summary = self.content.get(index)?;
        let selected = self.is_selected(summary.filename_hash);
        Some(self.render_summary(
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

fn get_minecraft_color(code: &str) -> Hsla {
    match code {
        "§0" => BLACK,
        "§1" => DARK_BLUE,
        "§2" => DARK_GREEN,
        "§3" => DARK_AQUA,
        "§4" => DARK_RED,
        "§5" => DARK_PURPLE,
        "§6" => GOLD,
        "§7" => GRAY,
        "§8" => DARK_GRAY,
        "§9" => BLUE,
        "§a" => LIME,
        "§b" => AQUA,
        "§c" => RED,
        "§d" => LIGHT_PURPLE,
        "§e" => YELLOW,
        "§f" => WHITE,
        _ => WHITE, // Includes §r (reset)
    }
}

fn parse_minecraft_text_color(text: &str) -> Div {
    let re = Regex::new(r"(§.)|([^§]+)").unwrap();

    let mut current_color = WHITE;
    let mut lines: Vec<Vec<Div>> = vec![Vec::new()];

    for mat in re.find_iter(text) {
        let part = mat.as_str();

        if part.starts_with('§') {
            current_color = get_minecraft_color(part);
        } else {
            for (i, segment) in part.split('\n').enumerate() {
                if i > 0 {
                    lines.push(Vec::new());
                }

                if !segment.is_empty() {
                    lines.last_mut().unwrap().push(
                        div().child(Label::new(SharedString::from(segment.to_string())).text_color(current_color)),
                    );
                }
            }
        }
    }

    div()
        .flex()
        .flex_col()
        .children(lines.into_iter().map(|line| div().flex().children(line)))
}

fn create_descriptions(
    name: &Option<Arc<str>>,
    version: &Arc<str>,
    authors: &Arc<str>,
    filename: &Arc<str>,
) -> (Div, Option<Div>) {
    // Creates the text for resource packs.
    if name.is_none() && authors.is_empty() {
        let description1 = v_flex()
            .w_2_5()
            .text_ellipsis()
            .child(SharedString::from(filename))
            .child(parse_minecraft_text_color(version));
        return (description1, None);
    }

    // Creates the text for everything else

    let description1 = v_flex()
        .w_1_5()
        .text_ellipsis()
        .child(SharedString::from(name.clone().unwrap_or(filename.clone())))
        .child(SharedString::from(version));

    const GRAY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.5,
        a: 1.0,
    };

    let mut description2 = v_flex().text_color(GRAY).child(SharedString::from(authors));

    if name.is_some() {
        description2 = description2.child(SharedString::from(filename));
    }

    (description1, Some(description2))
}
