use std::{collections::{HashMap, VecDeque}, path::Path, sync::Arc};

use bridge::{
    handle::BackendHandle,
    instance::{InstanceID, InstanceScreenshotSummary},
    message::{BridgeDataLoadState, MessageToBackend},
    serial::AtomicOptionSerial,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Colorize, Disableable, Sizable, button::{Button, ButtonVariants}, h_flex, input::{Input, InputEvent, InputState}, scroll::ScrollableElement, spinner::Spinner, v_flex
};
use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use schema::unique_bytes::UniqueBytes;

use crate::{
    entity::{DataEntities, instance::InstanceEntry}, icon::PandoraIcon, png_render_cache,
};

pub struct InstanceScreenshotsSubpage {
    instance: Entity<InstanceEntry>,
    backend_handle: BackendHandle,
    screenshots_state: BridgeDataLoadState,
    screenshots_serial: AtomicOptionSerial,
    thumbs: HashMap<Arc<Path>, Option<UniqueBytes>>,
    thumb_queue: VecDeque<Arc<Path>>,
    thumb_task: Option<Task<()>>,
    search_state: Entity<InputState>,
    search_query: String,
    viewer: Option<usize>,
    full_path: Option<Arc<Path>>,
    full_image: Option<UniqueBytes>,
    full_task: Option<Task<()>>,
    confirming_delete: Arc<Mutex<FxHashSet<Arc<Path>>>>,
}

impl InstanceScreenshotsSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        data: &DataEntities,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let instance_entity = instance.clone();
        let instance = instance.read(cx);

        let screenshots_state = instance.screenshots_state.clone();
        let screenshots = instance.screenshots.clone();

        let search_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t::common::search())
        });
        cx.subscribe(&search_state, Self::on_search).detach();

        cx.observe(&screenshots, |this, screenshots, cx| {
            let list = screenshots.read(cx).clone().map(|l| l.to_vec()).unwrap_or_default();
            this.queue_missing_thumbs(&list, cx);
            cx.notify();
        })
        .detach();

        cx.observe(&instance_entity, |_, _, cx| {
            cx.notify();
        })
        .detach();

        let mut this = Self {
            instance: instance_entity,
            backend_handle: data.backend_handle.clone(),
            screenshots_state,
            screenshots_serial: AtomicOptionSerial::default(),
            thumbs: HashMap::new(),
            thumb_queue: VecDeque::new(),
            thumb_task: None,
            search_state,
            search_query: String::new(),
            viewer: None,
            full_path: None,
            full_image: None,
            full_task: None,
            confirming_delete: Default::default(),
        };

        // The list may already be loaded (e.g. reopening the tab), in which
        // case no update arrives to trigger thumbnail loading
        let existing = screenshots.read(cx).clone().map(|l| l.to_vec()).unwrap_or_default();
        this.queue_missing_thumbs(&existing, cx);

        this
    }

    fn on_search(&mut self, entity: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        if let InputEvent::Change = event {
            self.search_query = entity.read(cx).value().to_string();
            cx.notify();
        }
    }

    fn queue_missing_thumbs(&mut self, screenshots: &[InstanceScreenshotSummary], cx: &mut Context<Self>) {
        self.thumbs.retain(|path, _| screenshots.iter().any(|screenshot| &screenshot.path == path));
        let mut queued = false;
        for screenshot in screenshots {
            if !self.thumbs.contains_key(&screenshot.path) && !self.thumb_queue.contains(&screenshot.path) {
                self.thumb_queue.push_back(screenshot.path.clone());
                queued = true;
            }
        }
        if queued {
            self.maybe_start_thumb(cx);
        }
    }

    fn maybe_start_thumb(&mut self, cx: &mut Context<Self>) {
        if self.thumb_task.is_some() {
            return;
        }
        let Some(path) = self.thumb_queue.pop_front() else {
            return;
        };

        let (send, recv) = tokio::sync::oneshot::channel::<(Arc<Path>, Option<UniqueBytes>)>();
        cx.background_executor().spawn(async move {
            let thumb = std::fs::read(path.as_ref()).ok()
                .and_then(|bytes| image::load_from_memory(&bytes).ok())
                .map(|image| image.resize(320, 320, image::imageops::FilterType::Lanczos3))
                .and_then(|thumb| {
                    let mut buf = Vec::new();
                    thumb.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).ok()?;
                    Some(UniqueBytes::from(buf))
                });
            let _ = send.send((path, thumb));
        }).detach();

        self.thumb_task = Some(cx.spawn(async move |this, cx| {
            let Ok((path, thumb)) = recv.await else {
                let _ = this.update(cx, |this, cx| {
                    this.thumb_task = None;
                    this.maybe_start_thumb(cx);
                    cx.notify();
                });
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.thumbs.insert(path, thumb);
                this.thumb_task = None;
                this.maybe_start_thumb(cx);
                cx.notify();
            });
        }));
    }

    fn open_viewer(&mut self, index: usize, path: Arc<Path>, cx: &mut Context<Self>) {
        self.viewer = Some(index);
        self.full_path = Some(path.clone());
        self.full_image = None;

        let (send, recv) = tokio::sync::oneshot::channel::<(Arc<Path>, Option<UniqueBytes>)>();
        cx.background_executor().spawn(async move {
            // Re-encode so the render cache always gets a png
            let image = std::fs::read(path.as_ref()).ok()
                .and_then(|bytes| image::load_from_memory(&bytes).ok())
                .and_then(|image| {
                    let mut buf = Vec::new();
                    image.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).ok()?;
                    Some(UniqueBytes::from(buf))
                });
            let _ = send.send((path, image));
        }).detach();

        self.full_task = Some(cx.spawn(async move |this, cx| {
            let Ok((path, image)) = recv.await else {
                let _ = this.update(cx, |this, cx| {
                    this.full_task = None;
                    cx.notify();
                });
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if this.full_path.as_ref() == Some(&path) {
                    this.full_image = image;
                }
                this.full_task = None;
                cx.notify();
            });
        }));
    }

    fn delete_button(&self, id: InstanceID, path: &Arc<Path>, button_id: impl Into<ElementId>, cx: &Context<Self>) -> Button {
        if self.confirming_delete.lock().contains(path) {
            let backend_handle = self.backend_handle.clone();
            let confirming_delete = self.confirming_delete.clone();
            let delete_path = path.clone();
            Button::new(button_id).danger().icon(PandoraIcon::Check).on_click(cx.listener(move |_, _, _, cx| {
                cx.stop_propagation();
                confirming_delete.lock().remove(&delete_path);
                backend_handle.send(MessageToBackend::DeleteScreenshots {
                    id,
                    path: delete_path.clone(),
                });
                cx.notify();
            }))
        } else {
            let confirming_delete = self.confirming_delete.clone();
            let arm_path = path.clone();
            Button::new(button_id).ghost().icon(PandoraIcon::Trash2).on_click(cx.listener(move |_, _, _, cx| {
                cx.stop_propagation();
                confirming_delete.lock().clear();
                confirming_delete.lock().insert(arm_path.clone());
                cx.notify();
            }))
        }
    }
}

impl Render for InstanceScreenshotsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let muted_foreground = cx.theme().muted_foreground;
        let instance = self.instance.read(cx);
        let instance_id = instance.id;

        self.screenshots_state.set_observed();
        if self.screenshots_state.should_load() {
            self.backend_handle
                .send_with_serial(MessageToBackend::RequestLoadScreenshots { id: instance_id }, &self.screenshots_serial);
        }

        let all_screenshots = instance.screenshots.read(cx).clone().map(|l| l.to_vec());
        let Some(all_screenshots) = all_screenshots else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(Spinner::new().color(muted_foreground).with_size(px(36.0)))
                .into_any_element();
        };

        let no_screenshots_at_all = all_screenshots.is_empty();
        let screenshots: Vec<InstanceScreenshotSummary> = if self.search_query.is_empty() {
            all_screenshots
        } else {
            all_screenshots.into_iter().filter(|s| s.filename.contains(&*self.search_query)).collect()
        };

        // The viewed file can move or disappear when the list reloads,
        // so track it by path instead of trusting the index
        if self.viewer.is_some() {
            let position = self.full_path.clone().and_then(|path| {
                screenshots.iter().position(|screenshot| screenshot.path == path)
            });
            match position {
                Some(index) => {
                    self.viewer = Some(index);
                    return self.render_viewer(&screenshots, index, cx).into_any_element();
                },
                None => {
                    self.viewer = None;
                    self.full_image = None;
                    self.full_path = None;
                },
            }
        }

        if screenshots.is_empty() {
            let message = if no_screenshots_at_all {
                t::instance::screenshots::empty()
            } else {
                t::common::no_results()
            };
            return v_flex()
                .size_full()
                .child(div().p_4().pb_0().child(Input::new(&self.search_state).prefix(PandoraIcon::Search).w_full()))
                .child(v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(div().child(PandoraIcon::Image).text_color(muted_foreground))
                    .child(div().text_color(muted_foreground).child(message)))
                .into_any_element();
        }

        let radius = cx.theme().radius;
        let border = cx.theme().border;
        let input = cx.theme().input;
        let transparent = cx.theme().transparent;
        let cards = screenshots.iter().enumerate().map(|(index, screenshot)| {
            let thumb: AnyElement = match self.thumbs.get(&screenshot.path) {
                Some(Some(bytes)) => png_render_cache::render(bytes.clone(), cx)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element(),
                Some(None) => gpui::img(ImageSource::Resource(Resource::Embedded("images/missing.png".into())))
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new().color(muted_foreground))
                    .into_any_element(),
            };

            let group_name: SharedString = format!("screenshot-{index}").into();
            let path = screenshot.path.clone();
            let open_path = path.clone();
            let folder_path = path.clone();
            let delete_path = path.clone();
            let copy_path = path.clone();
            let filename: SharedString = SharedString::from(screenshot.filename.clone());
            let id = instance_id;

            let is_confirming = self.confirming_delete.lock().contains(&delete_path);

            v_flex()
                .id(("screenshot-card", index))
                .group(group_name.clone())
                .gap_1()
                .p_2()
                .rounded(radius)
                .border_1()
                .border_color(border)
                .hover(|style| style.bg(input.mix_oklab(transparent, 0.5)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.confirming_delete.lock().clear();
                    this.open_viewer(index, open_path.clone(), cx);
                }))
                .child(div()
                    .relative()
                    .w_full()
                    .aspect_ratio(16.0 / 9.0)
                    .rounded(radius)
                    .overflow_hidden()
                    .bg(input.mix_oklab(transparent, 0.5))
                    .child(thumb)
                    .child(div()
                        .absolute()
                        .right_1()
                        .top_1()
                        .when(!is_confirming, |this| {
                            this.invisible().group_hover(group_name.clone(), |style| style.visible())
                        })
                        .child(h_flex()
                            .gap_0p5()
                            .p_0p5()
                            .rounded(radius)
                            .bg(cx.theme().background.opacity(0.85))
                            .border_1()
                            .border_color(border.opacity(0.6))
                            .child(Button::new(("screenshot-folder", index))
                                .ghost()
                                .compact()
                                .small()
                                .icon(PandoraIcon::FolderOpen)
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(parent) = folder_path.parent() {
                                        crate::open_folder(parent, window, cx);
                                    }
                                }))
                            .child(Button::new(("screenshot-copy", index))
                                .ghost()
                                .compact()
                                .small()
                                .icon(PandoraIcon::Copy)
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    copy_image(&copy_path, cx);
                                }))
                            .child(self.delete_button(id, &delete_path, ("screenshot-delete", index), cx)
                                .compact()
                                .small()))))
                .child(div().w_full().px_0p5().text_xs().truncate().child(filename))
                .into_any_element()
        }).collect::<Vec<_>>();

        v_flex()
            .size_full()
            .child(div().p_4().pb_0().child(Input::new(&self.search_state).prefix(PandoraIcon::Search).w_full()))
            .child(div().flex_1().overflow_y_scrollbar().child(div().p_4().child(
                div().grid().grid_cols(4).w_full().gap_4().children(cards)
            )))
            .into_any_element()
    }
}

impl InstanceScreenshotsSubpage {
    fn render_viewer(&mut self, screenshots: &[InstanceScreenshotSummary], index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let screenshot = &screenshots[index];

        let image = self.full_image.clone().map(|bytes| {
            png_render_cache::render(bytes, cx)
        });

        let theme = cx.theme();
        let filename: SharedString = SharedString::from(screenshot.filename.clone());
        let delete_path = screenshot.path.clone();
        let folder_path = screenshot.path.clone();
        let prev_path = index.checked_sub(1).and_then(|index| screenshots.get(index)).map(|screenshot| screenshot.path.clone());
        let next_path = screenshots.get(index + 1).map(|screenshot| screenshot.path.clone());
        let id = self.instance.read(cx).id;

        let viewer = v_flex().size_full().flex_1().min_h_0();

        let header = h_flex()
            .px_2()
            .pt_2()
            .gap_2()
            .items_center()
            .child(Button::new("back").icon(PandoraIcon::ArrowLeft).on_click(cx.listener(|this, _, _, cx| {
                this.viewer = None;
                this.full_image = None;
                this.full_path = None;
                cx.notify();
            })))
            .child(div().flex_1().overflow_x_hidden().whitespace_nowrap().child(filename.clone()))
            .child(Button::new("open-folder").icon(PandoraIcon::FolderOpen).on_click(move |_, window, cx| {
                if let Some(parent) = folder_path.parent() {
                    crate::open_folder(parent, window, cx);
                }
            }))
            .child(Button::new("copy").icon(PandoraIcon::Copy).on_click({
                let copy_path = screenshot.path.clone();
                move |_, _, cx| {
                    copy_image(&copy_path, cx);
                }
            }))
            .child(self.delete_button(id, &delete_path, "delete", cx));

        let body: AnyElement = if let Some(image) = image {
            div()
                .flex_1()
                .min_h_0()
                .p_4()
                .overflow_hidden()
                .child(image.size_full().object_fit(ObjectFit::Contain))
                .into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .child(Spinner::new().color(theme.muted_foreground).with_size(px(36.0)))
                .into_any_element()
        };

        let footer = h_flex()
            .px_4()
            .pb_4()
            .pt_1()
            .gap_2()
            .items_center()
            .justify_center()
            .child(Button::new("prev")
                .icon(PandoraIcon::ChevronLeft)
                .disabled(index == 0)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(path) = prev_path.clone() {
                        this.open_viewer(index - 1, path, cx);
                    }
                })))
            .child(format!("{} / {}", index + 1, screenshots.len()))
            .child(Button::new("next")
                .icon(PandoraIcon::ChevronRight)
                .disabled(index + 1 >= screenshots.len())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(path) = next_path.clone() {
                        this.open_viewer(index + 1, path, cx);
                    }
                })));

        viewer.child(header).child(body).child(footer)
    }
}

fn copy_image(path: &Arc<Path>, cx: &mut App) {
    let mut paths = ExternalPaths::default();
    paths.0.push(path.to_path_buf());
    cx.write_to_clipboard(ClipboardItem {
        entries: vec![ClipboardEntry::ExternalPaths(paths)],
    });
}
