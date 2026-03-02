use std::sync::Arc;

use bridge::{
    handle::BackendHandle,
    message::{MessageToBackend, PlayerSkinResult, SkinHistoryEntry, SkinModel},
};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    spinner::Spinner,
    v_flex,
    ActiveTheme as _,
    Disableable,
    Icon,
    Sizable,
    StyledExt,
};
use image::Frame;

use crate::{
    component::page::Page,
    entity::DataEntities,
    icon::PandoraIcon,
    png_render_cache,
    skin_renderer,
    ts,
};

pub struct ProfilePage {
    data: DataEntities,
    backend_handle: BackendHandle,

    // Import skin
    search_input: Entity<InputState>,
    is_fetching: bool,
    fetch_result: Option<Result<PlayerSkinResult, Arc<str>>>,

    // 3D rendered preview of fetched skin (cached PNG)
    fetch_3d_render: Option<Arc<[u8]>>,

    // Current account's 3D skin view
    current_skin_png: Option<Arc<[u8]>>,       // raw 64x64 skin texture (for re-rendering on rotation)
    current_skin_3d: Option<Arc<RenderImage>>,  // rendered 3D view (direct GPUI image, no PNG roundtrip)
    is_loading_current_skin: bool,

    // Rotation state for the current skin viewer
    skin_yaw: f64,
    skin_pitch: f64,
    is_dragging_skin: bool,
    drag_start_pos: Point<Pixels>,
    drag_start_yaw: f64,
    drag_start_pitch: f64,

    // Skin apply state
    is_applying: bool,
    apply_error: Option<Arc<str>>,

    // Skin history
    skin_history: Option<Vec<SkinHistoryEntry>>,
    _load_history_task: Task<()>,
    _load_skin_task: Task<()>,
    _apply_task: Task<()>,
}

impl ProfilePage {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(ts!("profile.username_placeholder"))
        });

        let mut page = Self {
            data: data.clone(),
            backend_handle: data.backend_handle.clone(),
            search_input,
            is_fetching: false,
            fetch_result: None,
            fetch_3d_render: None,
            current_skin_png: None,
            current_skin_3d: None,
            is_loading_current_skin: false,
            skin_yaw: skin_renderer::DEFAULT_YAW,
            skin_pitch: skin_renderer::DEFAULT_PITCH,
            is_dragging_skin: false,
            drag_start_pos: Point::default(),
            drag_start_yaw: 0.0,
            drag_start_pitch: 0.0,
            is_applying: false,
            apply_error: None,
            skin_history: None,
            _load_history_task: Task::ready(()),
            _load_skin_task: Task::ready(()),
            _apply_task: Task::ready(()),
        };

        page.load_skin_history(cx);
        page.load_current_skin_3d(cx);

        page
    }

    fn load_skin_history(&mut self, cx: &mut Context<Self>) {
        let accounts = self.data.accounts.read(cx);
        let Some(account) = &accounts.selected_account else {
            return;
        };
        let uuid = account.uuid;

        let (send, recv) = tokio::sync::oneshot::channel();
        self.backend_handle.send(MessageToBackend::GetSkinHistory {
            account_uuid: uuid,
            channel: send,
        });

        self._load_history_task = cx.spawn(async move |page, cx| {
            let Ok(entries) = recv.await else { return };
            let _ = page.update(cx, move |page, cx| {
                page.skin_history = Some(entries);
                cx.notify();
            });
        });
    }

    /// Auto-fetch the current account's skin by username for 3D rendering.
    fn load_current_skin_3d(&mut self, cx: &mut Context<Self>) {
        let accounts = self.data.accounts.read(cx);
        let Some(account) = &accounts.selected_account else {
            return;
        };
        let username: Arc<str> = Arc::from(&*account.username);

        self.is_loading_current_skin = true;
        cx.notify();

        let (send, recv) = tokio::sync::oneshot::channel();
        self.backend_handle.send(MessageToBackend::FetchPlayerSkin {
            username,
            channel: send,
        });

        let yaw = self.skin_yaw;
        let pitch = self.skin_pitch;

        self._load_skin_task = cx.spawn(async move |page, cx| {
            let Ok(result) = recv.await else { return };
            let _ = page.update(cx, move |page, cx| {
                page.is_loading_current_skin = false;
                if let Ok(skin_result) = result {
                    page.current_skin_png = Some(skin_result.skin_png.clone());
                    // Render the 3D view from the full skin texture
                    page.current_skin_3d =
                        Self::skin_to_render_image(&skin_result.skin_png, 200, 360, yaw, pitch);
                }
                cx.notify();
            });
        });
    }

    /// Re-render the current skin 3D view with the current rotation angles.
    fn re_render_current_skin(&mut self) {
        if let Some(skin_png) = &self.current_skin_png {
            self.current_skin_3d =
                Self::skin_to_render_image(skin_png, 200, 360, self.skin_yaw, self.skin_pitch);
        }
    }

    /// Render a skin texture to a GPUI `RenderImage` at the given rotation.
    /// Bypasses PNG encoding for fast interactive rendering.
    fn skin_to_render_image(
        skin_png: &[u8],
        width: u32,
        height: u32,
        yaw: f64,
        pitch: f64,
    ) -> Option<Arc<RenderImage>> {
        let mut output = skin_renderer::render_skin_3d_raw(skin_png, width, height, yaw, pitch)?;
        // Convert RGBA to BGRA for GPUI
        for pixel in output.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Some(Arc::new(RenderImage::new([Frame::new(output)])))
    }

    fn fetch_skin(&mut self, cx: &mut Context<Self>) {
        let username = self.search_input.read(cx).value();
        let username = username.trim();
        if username.is_empty() {
            return;
        }

        self.is_fetching = true;
        self.fetch_result = None;
        self.fetch_3d_render = None;
        cx.notify();

        let username: Arc<str> = Arc::from(username);
        let (send, recv) = tokio::sync::oneshot::channel();

        self.backend_handle.send(MessageToBackend::FetchPlayerSkin {
            username,
            channel: send,
        });

        cx.spawn(async move |page, cx| {
            let Ok(result) = recv.await else { return };
            let _ = page.update(cx, move |page, cx| {
                page.is_fetching = false;
                // Pre-render 3D view for fetched skin
                if let Ok(ref skin_result) = result {
                    if let Some(rendered) = skin_renderer::render_skin_3d(
                        &skin_result.skin_png,
                        160,
                        280,
                        skin_renderer::DEFAULT_YAW,
                        skin_renderer::DEFAULT_PITCH,
                    ) {
                        page.fetch_3d_render = Some(Arc::from(rendered));
                    }
                }
                page.fetch_result = Some(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_skin(&mut self, head_png: Arc<[u8]>, skin_png: Arc<[u8]>, skin_model: SkinModel, source_name: Arc<str>, cx: &mut Context<Self>) {
        let accounts = self.data.accounts.read(cx);
        let Some(account) = &accounts.selected_account else {
            return;
        };
        let uuid = account.uuid;

        self.is_applying = true;
        self.apply_error = None;
        cx.notify();

        let (send, recv) = tokio::sync::oneshot::channel();
        self.backend_handle.send(MessageToBackend::ApplySkin {
            account_uuid: uuid,
            head_png,
            skin_png: skin_png.clone(),
            skin_model,
            source_name,
            channel: send,
        });

        let skin_png_for_render = skin_png;
        self._apply_task = cx.spawn(async move |page, cx| {
            let Ok(result) = recv.await else { return };
            let _ = page.update(cx, move |page, cx| {
                page.is_applying = false;
                match result {
                    Ok(()) => {
                        page.apply_error = None;
                        // Update the 3D skin view with the applied skin's texture
                        page.current_skin_png = Some(skin_png_for_render);
                        page.skin_yaw = skin_renderer::DEFAULT_YAW;
                        page.skin_pitch = skin_renderer::DEFAULT_PITCH;
                        page.re_render_current_skin();
                        // Reload history after applying
                        page.load_skin_history(cx);
                    },
                    Err(e) => {
                        page.apply_error = Some(e);
                    },
                }
                cx.notify();
            });
        });
    }

    fn apply_skin_from_history(&mut self, index: usize, cx: &mut Context<Self>) {
        let accounts = self.data.accounts.read(cx);
        let Some(account) = &accounts.selected_account else {
            return;
        };
        let uuid = account.uuid;

        // Get the skin data from history for local preview update on success
        let history_entry = self.skin_history.as_ref()
            .and_then(|h| h.get(index))
            .cloned();

        self.is_applying = true;
        self.apply_error = None;
        cx.notify();

        let (send, recv) = tokio::sync::oneshot::channel();
        self.backend_handle.send(MessageToBackend::ApplySkinFromHistory {
            account_uuid: uuid,
            history_index: index,
            channel: send,
        });

        self._apply_task = cx.spawn(async move |page, cx| {
            let Ok(result) = recv.await else { return };
            let _ = page.update(cx, move |page, cx| {
                page.is_applying = false;
                match result {
                    Ok(()) => {
                        page.apply_error = None;
                        // Update 3D view if we have the skin data
                        if let Some(entry) = history_entry {
                            if !entry.skin_png.is_empty() {
                                page.current_skin_png = Some(entry.skin_png);
                                page.skin_yaw = skin_renderer::DEFAULT_YAW;
                                page.skin_pitch = skin_renderer::DEFAULT_PITCH;
                                page.re_render_current_skin();
                            } else {
                                // Reload from server if no local skin data
                                page.load_current_skin_3d(cx);
                            }
                        }
                        page.load_skin_history(cx);
                    },
                    Err(e) => {
                        page.apply_error = Some(e);
                    },
                }
                cx.notify();
            });
        });
    }
}

impl Render for ProfilePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts = self.data.accounts.read(cx);
        let selected_account = accounts.selected_account.clone();

        let Some(account) = selected_account else {
            let content = v_flex()
                .size_full()
                .p_6()
                .gap_4()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(PandoraIcon::CircleUser)
                        .size_16()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .text_lg()
                        .text_color(cx.theme().muted_foreground)
                        .child(ts!("profile.select_account_first")),
                );

            return Page::new(ts!("profile.title")).scrollable().child(content);
        };

        // Current account head (2D)
        let account_head = if let Some(head) = &account.head {
            let resize =
                png_render_cache::ImageTransformation::Resize {
                    width: 64,
                    height: 64,
                };
            png_render_cache::render_with_transform(Arc::clone(head), resize, cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded(
                "images/default_head.png".into(),
            )))
        };

        let account_name = SharedString::new(account.username.clone());
        let account_uuid_str = account.uuid.to_string();

        // 3D skin view (interactive: drag to rotate)
        let is_dragging = self.is_dragging_skin;
        let skin_3d_element = if let Some(render_image) = &self.current_skin_3d {
            let skin_img = gpui::img(render_image.clone());
            let mut viewer = div()
                .id("skin-3d-viewer")
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .p_3()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|page, event: &MouseDownEvent, _window, _cx| {
                        page.is_dragging_skin = true;
                        page.drag_start_pos = event.position;
                        page.drag_start_yaw = page.skin_yaw;
                        page.drag_start_pitch = page.skin_pitch;
                    }),
                )
                .on_mouse_move(cx.listener(|page, event: &MouseMoveEvent, _window, cx| {
                    if !page.is_dragging_skin {
                        return;
                    }
                    let dx = event.position.x.to_f64() - page.drag_start_pos.x.to_f64();
                    let dy = event.position.y.to_f64() - page.drag_start_pos.y.to_f64();
                    page.skin_yaw = page.drag_start_yaw + dx * 0.5;
                    page.skin_pitch = (page.drag_start_pitch + dy * 0.3).clamp(-60.0, 60.0);
                    page.re_render_current_skin();
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|page, _event: &MouseUpEvent, _window, _cx| {
                        page.is_dragging_skin = false;
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|page, _event: &MouseUpEvent, _window, _cx| {
                        page.is_dragging_skin = false;
                    }),
                );
            if is_dragging {
                viewer = viewer.cursor_grabbing();
            } else {
                viewer = viewer.cursor_grab();
            }
            viewer
                .child(skin_img.h(px(280.0)).w(px(155.0)))
                .into_any_element()
        } else if self.is_loading_current_skin {
            div()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .p_3()
                .w(px(155.0))
                .h(px(280.0))
                .items_center()
                .justify_center()
                .child(Spinner::new())
                .into_any_element()
        } else {
            // No 3D view available (e.g. offline account)
            div()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .p_3()
                .w(px(155.0))
                .h(px(280.0))
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(PandoraIcon::CircleUser)
                                .size_12()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(ts!("profile.no_3d_preview")),
                        ),
                )
                .into_any_element()
        };

        // Account info (to the right of 3D view)
        let account_info = v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .p_2()
                            .child(account_head.size_16().min_w_16().min_h_16()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .child(account_name),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(ts!("profile.uuid_label", uuid = account_uuid_str)),
                            ),
                    ),
            );

        let current_skin_section = v_flex()
            .gap_3()
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .font_semibold()
                    .child(ts!("profile.current_skin")),
            )
            .child(
                h_flex()
                    .gap_6()
                    .items_start()
                    .child(skin_3d_element)
                    .child(account_info),
            );

        // Import skin section
        let search_button = if self.is_fetching {
            Button::new("fetch-skin")
                .label(ts!("profile.fetching"))
                .disabled(true)
                .child(Spinner::new())
        } else {
            Button::new("fetch-skin")
                .label(ts!("profile.fetch"))
                .icon(PandoraIcon::Search)
                .info()
                .on_click(cx.listener(|page, _, _, cx| {
                    page.fetch_skin(cx);
                }))
        };

        let mut import_section = v_flex()
            .gap_3()
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .font_semibold()
                    .child(ts!("profile.import_skin")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(ts!("profile.import_description")),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(Input::new(&self.search_input).max_w_80())
                    .child(search_button),
            );

        // Show fetch result / preview
        if let Some(result) = &self.fetch_result {
            match result {
                Ok(skin_result) => {
                    let preview_head = {
                        let resize = png_render_cache::ImageTransformation::Resize {
                            width: 64,
                            height: 64,
                        };
                        png_render_cache::render_with_transform(
                            Arc::clone(&skin_result.head_png),
                            resize,
                            cx,
                        )
                    };

                    let preview_name = SharedString::new(skin_result.username.clone());
                    let head_png = skin_result.head_png.clone();
                    let skin_png = skin_result.skin_png.clone();
                    let skin_model = skin_result.skin_model;
                    let source_name = skin_result.username.clone();

                    // Left side: 3D preview (if available)
                    let preview_3d = if let Some(rendered) = &self.fetch_3d_render {
                        let img = png_render_cache::render(Arc::clone(rendered), cx);
                        div()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .p_2()
                            .child(img.h(px(200.0)).w(px(110.0)))
                    } else {
                        div()
                            .rounded(cx.theme().radius)
                            .p_1()
                            .child(preview_head.size_12().min_w_12().min_h_12())
                    };

                    let preview = h_flex()
                        .gap_4()
                        .items_center()
                        .p_3()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(preview_3d)
                        .child(
                            v_flex()
                                .flex_grow()
                                .gap_0p5()
                                .child(div().font_semibold().child(preview_name))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(ts!("profile.preview")),
                                ),
                        )
                        .child(
                            if self.is_applying {
                                Button::new("apply-fetched")
                                    .label(ts!("profile.applying"))
                                    .disabled(true)
                                    .child(Spinner::new())
                                    .into_any_element()
                            } else {
                                Button::new("apply-fetched")
                                    .label(ts!("profile.apply"))
                                    .icon(PandoraIcon::Download)
                                    .success()
                                    .on_click(cx.listener(move |page, _, _, cx| {
                                        page.apply_skin(head_png.clone(), skin_png.clone(), skin_model, source_name.clone(), cx);
                                    }))
                                    .into_any_element()
                            },
                        );

                    import_section = import_section.child(preview);
                }
                Err(err) => {
                    let err_msg = ts!("profile.fetch_error", err = err);
                    import_section = import_section.child(
                        div()
                            .p_3()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().danger)
                            .text_color(cx.theme().danger)
                            .text_sm()
                            .child(err_msg),
                    );
                }
            }
        }

        // Skin history section
        let mut history_section = v_flex().gap_3().child(
            div()
                .border_b_1()
                .border_color(cx.theme().border)
                .text_lg()
                .font_semibold()
                .child(ts!("profile.skin_history")),
        );

        if let Some(history) = &self.skin_history {
            if history.is_empty() {
                history_section = history_section.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(ts!("profile.no_history")),
                );
            } else {
                for (index, entry) in history.iter().enumerate() {
                    let entry_head = {
                        let resize = png_render_cache::ImageTransformation::Resize {
                            width: 32,
                            height: 32,
                        };
                        png_render_cache::render_with_transform(
                            Arc::clone(&entry.head_png),
                            resize,
                            cx,
                        )
                    };

                    let source_label =
                        ts!("profile.from", name = entry.source_name.clone());

                    // Format timestamp
                    let time_str = {
                        let dt = chrono::DateTime::from_timestamp(entry.timestamp, 0);
                        if let Some(dt) = dt {
                            SharedString::from(
                                dt.format("%Y-%m-%d %H:%M").to_string(),
                            )
                        } else {
                            SharedString::from("Unknown")
                        }
                    };

                    let entry_row = h_flex()
                        .gap_3()
                        .items_center()
                        .p_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .rounded(cx.theme().radius)
                                .child(entry_head.size_8().min_w_8().min_h_8()),
                        )
                        .child(
                            v_flex()
                                .flex_grow()
                                .gap_0p5()
                                .child(div().text_sm().font_semibold().child(source_label))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(time_str),
                                ),
                        )
                        .child(
                            if self.is_applying {
                                Button::new(("revert", index))
                                    .label(ts!("profile.revert"))
                                    .small()
                                    .disabled(true)
                                    .into_any_element()
                            } else if entry.skin_png.is_empty() {
                                Button::new(("revert", index))
                                    .label(ts!("profile.revert"))
                                    .small()
                                    .disabled(true)
                                    .into_any_element()
                            } else {
                                Button::new(("revert", index))
                                    .label(ts!("profile.revert"))
                                    .small()
                                    .on_click(cx.listener(move |page, _, _, cx| {
                                        page.apply_skin_from_history(index, cx);
                                    }))
                                    .into_any_element()
                            },
                        );

                    history_section = history_section.child(entry_row);
                }
            }
        } else {
            history_section = history_section.child(Spinner::new());
        }

        let mut content = v_flex()
            .size_full()
            .p_6()
            .gap_6()
            .child(current_skin_section)
            .child(import_section);

        // Show apply error if any
        if let Some(error) = &self.apply_error {
            let err_msg = SharedString::from(format!("Failed to apply skin: {}", error));
            content = content.child(
                div()
                    .p_3()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().danger)
                    .text_color(cx.theme().danger)
                    .text_sm()
                    .child(err_msg),
            );
        }

        // Show applying indicator
        if self.is_applying {
            content = content.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Spinner::new())
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(ts!("profile.uploading_skin")),
                    ),
            );
        }

        content = content.child(history_section);

        Page::new(ts!("profile.title"))
            .scrollable()
            .child(content)
    }
}
