use std::sync::Arc;

use gpui::{App, AppContext, AvailableSpace, Bounds, Element, Entity, IntoElement, RenderImage, Size, Style, Task, px, size};
use schema::{minecraft_profile::SkinVariant, unique_bytes::UniqueBytes};

pub const DEFAULT_YAW: f64 = 22.5;
pub const DEFAULT_PITCH: f64 = 10.5;
pub const DEFAULT_ANIMATION: f64 = 1.0/16.0;

struct RenderedPlayerModel {
    image: Arc<RenderImage>,
    skin: UniqueBytes,
    cape: Option<UniqueBytes>,
    variant: SkinVariant,
    yaw: f64,
    pitch: f64,
    animation: f64,
    width: u32,
    height: u32,
}

pub struct PlayerModelState {
    pub skin: UniqueBytes,
    pub cape: Option<UniqueBytes>,
    pub variant: SkinVariant,
    pub yaw: f64,
    pub pitch: f64,
    pub animation: f64,
    rendered: Option<RenderedPlayerModel>,
    render_task: Option<Task<()>>,
}

impl PlayerModelState {
    pub fn new(cx: &mut App, skin: UniqueBytes, variant: SkinVariant) -> Entity<Self> {
        let entity = cx.new(|_| Self {
            skin,
            cape: None,
            variant,
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
        return rendered.width != width || rendered.height != height || rendered.yaw != self.yaw
            || rendered.pitch != self.pitch || rendered.animation != self.animation
            || rendered.variant != self.variant || rendered.skin != self.skin || rendered.cape != self.cape;
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
        let element_height = bounds.size.height.as_f32().round();
        let element_width = (element_height as f32 * crate::skin_renderer::ASPECT_RATIO as f32).round();
        let window_scale = window.scale_factor();
        let image_height = (element_height * window_scale) as u32;
        let image_width = (element_width * window_scale) as u32;
        self.state.update(cx, |state, cx| {
            if state.render_task.is_none() && state.needs_rerender(image_width, image_height) {
                let skin = state.skin.clone();
                let cape = state.cape.clone();
                let yaw = state.yaw;
                let pitch = state.pitch;
                let animation = state.animation;
                let variant = state.variant;

                let (send, recv) = tokio::sync::oneshot::channel();

                cx.background_executor().spawn(async move {
                    send.send(crate::skin_renderer::render_skin_3d(&skin, cape.as_deref(), variant, image_width, image_height, yaw, pitch, animation, 0.0, 1.0))
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
                            variant,
                            yaw,
                            pitch,
                            animation,
                            width: image_width,
                            height: image_height,
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
                        size: Size::new(px(element_width), px(element_height)),
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
