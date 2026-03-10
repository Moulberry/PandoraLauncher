use std::sync::Arc;

use gpui::{App, AppContext, AvailableSpace, Bounds, Element, Entity, IntoElement, RenderImage, Size, Style, Task, px, size};

pub const DEFAULT_YAW: f64 = 22.5;
pub const DEFAULT_PITCH: f64 = 10.5;
pub const DEFAULT_ANIMATION: f64 = 1.0/16.0;

struct RenderedPlayerModel {
    image: Arc<RenderImage>,
    skin: Arc<[u8]>,
    cape: Option<Arc<[u8]>>,
    yaw: f64,
    pitch: f64,
    animation: f64,
    width: u32,
    height: u32,
}

pub struct PlayerModelState {
    pub skin: Arc<[u8]>,
    pub cape: Option<Arc<[u8]>>,
    pub yaw: f64,
    pub pitch: f64,
    pub animation: f64,
    rendered: Option<RenderedPlayerModel>,
    render_task: Option<Task<()>>,
}

impl PlayerModelState {
    pub fn new(cx: &mut App, skin: Arc<[u8]>) -> Entity<Self> {
        let entity = cx.new(|_| Self {
            skin,
            cape: None,
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            animation: DEFAULT_ANIMATION,
            rendered: None,
            render_task: None,
        });
        cx.observe_release(&entity, |entity, cx| {
            if let Some(rendered) = entity.rendered.take() {
                cx.drop_image(rendered.image, None);
            }
        }).detach();
        entity
    }

    pub fn needs_rerender(&self, width: u32, height: u32) -> bool {
        let Some(rendered) = &self.rendered else {
            return true;
        };
        if rendered.width != width || rendered.height != height || rendered.yaw != self.yaw
            || rendered.pitch != self.pitch || rendered.animation != self.animation
            || !Arc::ptr_eq(&rendered.skin, &self.skin)
        {
                return true;
        }
        if let Some(rendered_cape) = &rendered.cape {
            if let Some(self_cape) = &self.cape {
                !Arc::ptr_eq(rendered_cape, self_cape)
            } else {
                true
            }
        } else {
            self.cape.is_some()
        }
    }
}

pub struct PlayerModel {
    state: Entity<PlayerModelState>,
}

impl PlayerModel {
    pub fn new(state: &Entity<PlayerModelState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl IntoElement for PlayerModel {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PlayerModel {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let layout_id = window.request_measured_layout(Style::default(), move |known, available_space, _window, _cx| {
            let height = if let Some(height) = known.height {
                height
            } else {
                match available_space.height {
                    AvailableSpace::Definite(pixels) => pixels,
                    AvailableSpace::MinContent => px(0.0),
                    AvailableSpace::MaxContent => px(1000.0),
                }
            };

            let width = px(height.as_f32() * crate::skin_renderer::ASPECT_RATIO as f32);

            size(width, height)
        });

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _element_size: &mut Self::RequestLayoutState,
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _global_id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let height = bounds.size.height.as_f32() as u32;
        let width = (height as f32 * crate::skin_renderer::ASPECT_RATIO as f32) as u32;
        self.state.update(cx, |state, cx| {
            if state.render_task.is_none() && state.needs_rerender(width, height) {
                let skin = state.skin.clone();
                let cape = state.cape.clone();
                let yaw = state.yaw;
                let pitch = state.pitch;
                let animation = state.animation;

                let (send, recv) = tokio::sync::oneshot::channel();

                cx.background_executor().spawn(async move {
                    send.send(crate::skin_renderer::render_skin_3d(&skin, cape.as_deref(), width, height, yaw, pitch, animation))
                }).detach();

                let skin = state.skin.clone();
                let cape = state.cape.clone();
                state.render_task = Some(cx.spawn(async move |state, cx| {
                    let Ok(Some(mut data)) = recv.await else {
                        return;
                    };

                    _ = state.update(cx, |state, cx| {
                        for pixel in data.chunks_exact_mut(4) {
                            pixel.swap(0, 2);
                        }

                        let render_image = Arc::new(RenderImage::new([image::Frame::new(data)]));

                        if let Some(rendered) = state.rendered.take() {
                            cx.drop_image(rendered.image, None);
                        }
                        state.rendered = Some(RenderedPlayerModel {
                            image: render_image,
                            skin,
                            cape,
                            yaw,
                            pitch,
                            animation,
                            width,
                            height,
                        });
                        state.render_task = None;
                        cx.notify();
                    });
                }));
            }

            if let Some(rendered) = &state.rendered {
                _ = window.paint_image(
                    Bounds {
                        origin: bounds.origin,
                        size: Size::new(px(width as f32), px(height as f32)),
                    },
                    Default::default(),
                    rendered.image.clone(),
                    0,
                    false,
                );
            }
        });
    }
}
