use gpui::{App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Size, Style, Window};

pub struct ShrinkingText(SharedString);

impl ShrinkingText {
    pub fn new(text: SharedString) -> Self {
        Self(text)
    }
}

impl IntoElement for ShrinkingText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ShrinkingText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = text_style.line_height.to_pixels(font_size.into(), window.rem_size());

        let shaped = window.text_system().shape_line(self.0.clone(), font_size,
            &[text_style.to_run(self.0.len())], None);

        let layout_id = window.request_measured_layout(Style::default(), {
            move |known_dimensions, available_space, _window, _cx| {
                let width = if let Some(width) = known_dimensions.width {
                    width
                } else {
                    match available_space.width {
                        AvailableSpace::Definite(pixels) => pixels,
                        AvailableSpace::MinContent => Pixels::ZERO,
                        AvailableSpace::MaxContent => shaped.width.ceil(),
                    }
                };
                Size::new(width, line_height)
            }
        });

        (layout_id, ())

    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        ()
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _text_layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = text_style.line_height.to_pixels(font_size.into(), window.rem_size());

        let full_shaped = window.text_system().shape_line(self.0.clone(), font_size,
            &[text_style.to_run(self.0.len())], None);

        let scale = bounds.size.width.as_f32().ceil() / full_shaped.width.as_f32().ceil();
        if scale >= 1.0 {
            _ = full_shaped.paint(bounds.origin, line_height, gpui::TextAlign::Left, None, window, cx);
        } else {
            let scaled_font_size = font_size * scale;
            let shaped = window.text_system().shape_line(self.0.clone(), scaled_font_size,
                &[text_style.to_run(self.0.len())], None);

            let baseline_offset1 = (line_height + full_shaped.ascent - full_shaped.descent) / 2.0;
            let baseline_offset2 = (line_height*scale + shaped.ascent - shaped.descent) / 2.0;

            let mut origin = bounds.origin;
            origin.y += baseline_offset1 - baseline_offset2;
            _ = shaped.paint(origin, line_height*scale, gpui::TextAlign::Left, None, window, cx);
        }
    }
}
