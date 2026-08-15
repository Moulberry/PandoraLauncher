use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, AvailableSpace, Bounds, ContentMask, DispatchPhase, Element, Hitbox, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Size, Style, Window, px};
use gpui_component::ActiveTheme;

const RESIZE_WIDTH: Pixels = px(1.0);
const RESIZE_PADDING: Pixels = px(4.0);

#[derive(Clone)]
pub struct ResizePanelState(Rc<RefCell<ResizePanelStateData>>);

struct ResizePanelStateData {
    drag_offset: Option<Pixels>,
    size: Pixels,
    ratio: Option<f32>,
    min_size: Pixels,
    max_size: Pixels,
    on_resize: Rc<dyn Fn(Pixels, &mut Window, &mut App)>,
}

impl ResizePanelState {
    pub fn new(initial_size: Pixels, min_size: Pixels, max_size: Pixels) -> Self {
        Self(Rc::new(RefCell::new(ResizePanelStateData {
            drag_offset: None,
            size: initial_size,
            ratio: None,
            min_size,
            max_size,
            on_resize: Rc::new(|_, _, _| {}),
        })))
    }

    pub fn on_resize(self, resize: impl Fn(Pixels, &mut Window, &mut App) + 'static) -> Self {
        self.0.borrow_mut().on_resize = Rc::new(resize);
        self
    }
}

pub struct ResizePanel {
    state: ResizePanelState,
    left: AnyElement,
    right: AnyElement,
}

impl ResizePanel {
    pub fn new(state: &ResizePanelState, left: impl IntoElement, right: impl IntoElement) -> Self {
        Self {
            state: state.clone(),
            left: left.into_any_element(),
            right: right.into_any_element(),
        }
    }
}


impl IntoElement for ResizePanel {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ResizePanel {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

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
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size = Size::full();
        let layout_id = window.request_layout(style, [], cx);

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        if bounds.size.width < RESIZE_WIDTH {
            return None;
        }

        let mut state = self.state.0.borrow_mut();
        if state.drag_offset.is_none() {
            if let Some(ratio) = state.ratio {
                let new_size = (bounds.size.width * ratio).round().clamp(state.min_size, state.max_size);
                if state.size != new_size {
                    state.size = new_size;
                    (state.on_resize)(state.size, window, cx);
                }
            } else {
                state.ratio = Some(state.size / bounds.size.width);
            }
        }
        let size = state.size.clamp(Pixels::ZERO, bounds.size.width - RESIZE_WIDTH);
        drop(state);

        let left_size = Size::new(AvailableSpace::Definite(size),
            AvailableSpace::Definite(bounds.size.height));
        self.left.layout_as_root(left_size, window, cx);
        self.left.prepaint_at(bounds.origin, window, cx);

        let right_size = Size::new(AvailableSpace::Definite(bounds.size.width - size - RESIZE_WIDTH),
            AvailableSpace::Definite(bounds.size.height));
        self.right.layout_as_root(right_size, window, cx);
        let mut right_origin = bounds.origin.clone();
        right_origin.x += size + RESIZE_WIDTH;
        self.right.prepaint_at(right_origin, window, cx);

        let mut line_origin = bounds.origin.clone();
        line_origin.x += size - RESIZE_PADDING;
        let line_bounds = Bounds {
            origin: line_origin,
            size: Size::new(RESIZE_PADDING*2 + RESIZE_WIDTH, bounds.size.height),
        };

        Some(window.insert_hitbox(line_bounds, gpui::HitboxBehavior::BlockMouse))
    }

    fn paint(
        &mut self,
        _global_id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let Some(hitbox) = hitbox else {
            return;
        };

        let state = self.state.0.borrow();
        let size = state.size.min(bounds.size.width - RESIZE_WIDTH);
        if state.drag_offset.is_some() {
            window.set_window_cursor_style(gpui::CursorStyle::ResizeLeftRight);
        }
        let is_dragging = state.drag_offset.is_some();
        drop(state);

        let mut left_bounds = bounds.clone();
        left_bounds.size.width = size;
        window.with_content_mask(Some(ContentMask { bounds: left_bounds }), |window| {
            self.left.paint(window, cx);
        });

        let mut right_bounds = bounds.clone();
        right_bounds.origin.x += size + RESIZE_WIDTH;
        right_bounds.size.width = bounds.size.width - size - RESIZE_WIDTH;
        window.with_content_mask(Some(ContentMask { bounds: right_bounds }), |window| {
            self.right.paint(window, cx);
        });

        let mut line_origin = bounds.origin.clone();
        line_origin.x += size;
        let line_bounds = Bounds {
            origin: line_origin,
            size: Size::new(RESIZE_WIDTH, bounds.size.height),
        };
        let border = if is_dragging {
            cx.theme().drag_border
        } else {
            cx.theme().border
        };
        window.paint_quad(gpui::fill(line_bounds, border));

        window.set_cursor_style(gpui::CursorStyle::ResizeLeftRight, hitbox);

        window.on_mouse_event({
            let state = self.state.0.clone();
            let line_x = line_origin.x;
            let hitbox = hitbox.clone();
            move |event: &MouseDownEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                {
                    let mut state = state.borrow_mut();
                    state.drag_offset = Some(event.position.x - line_x);
                    state.ratio = None;
                    window.refresh();
                }
            }
        });

        window.on_mouse_event({
            let state = self.state.0.clone();
            move |event: &MouseUpEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    let mut state = state.borrow_mut();
                    if state.drag_offset.is_some() {
                        state.drag_offset = None;
                        state.ratio = None;
                        window.refresh();
                    }
                }
            }
        });

        window.on_mouse_event({
            let state = self.state.0.clone();
            move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    return;
                }

                let mut state = state.borrow_mut();
                if let Some(drag_offset) = state.drag_offset
                    && !cx.has_active_drag()
                    && event.pressed_button == Some(MouseButton::Left)
                {
                    let new_size = (event.position.x - drag_offset).round().clamp(state.min_size, state.max_size);
                    if state.size != new_size {
                        state.size = new_size;
                        state.ratio = None;
                        (state.on_resize)(state.size, window, cx);
                        window.refresh();
                        cx.stop_propagation();
                    }
                }
            }
        });
    }
}
