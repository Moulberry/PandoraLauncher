use gpui::{px, size, AnyElement, AvailableSpace, DefiniteLength, Element, InteractiveElement, Interactivity, IntoElement, ParentElement, Pixels, Point, Size, StyleRefinement, Styled, UniformList};

pub struct ResponsiveGrid {
    interactivity: Interactivity,
    min_element_size: Size<AvailableSpace>,
    children: Vec<AnyElement>,
}

impl ResponsiveGrid {
    pub fn new(min_element_size: Size<AvailableSpace>) -> Self {
        Self {
            interactivity: Interactivity::default(),
            min_element_size,
            children: Vec::new(),
        }
    }
}

impl Styled for ResponsiveGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for ResponsiveGrid {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl ParentElement for ResponsiveGrid {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl IntoElement for ResponsiveGrid {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ResponsiveGrid {
    type RequestLayoutState = Size<Pixels>;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let (min_element_width, min_element_height) = if let AvailableSpace::Definite(def_width) = self.min_element_size.width
            && let AvailableSpace::Definite(def_height) = self.min_element_size.height
        {
            (def_width, def_height)
        } else {
            let mut size = self.children.iter_mut().map(|child| {
                let size = child.layout_as_root(self.min_element_size, window, cx);
                (size.width, size.height)
            }).reduce(|(w1, h1), (w2, h2)| (w1.max(w2), h1.max(h2))).unwrap_or_default();

            if let AvailableSpace::Definite(def_width) = self.min_element_size.width {
                size.0 = def_width;
            }
            if let AvailableSpace::Definite(def_height) = self.min_element_size.height {
                size.1 = def_height;
            }

            size
        };

        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, _cx| {
                let rem_size = window.rem_size();
                let font_size = window.text_style().font_size;
                let gap_width = style.gap.width.to_pixels(font_size, rem_size);
                let gap_height = style.gap.height.to_pixels(font_size, rem_size);
                let children_count = self.children.len();

                window.request_measured_layout(
                    style,
                    move |known, available_space, _window, _cx| {
                        let base_width = known.width.unwrap_or(match available_space.width {
                            AvailableSpace::Definite(pixels) => pixels,
                            AvailableSpace::MinContent | AvailableSpace::MaxContent => min_element_width,
                        });

                        let (width, horizontal_count) = if base_width <= min_element_width {
                            (base_width, 1)
                        } else {
                            let bounds_width_plus_padding = base_width.to_f64() + gap_width.to_f64();
                            let min_element_width_plus_padding = min_element_width.to_f64() + gap_width.to_f64();
                            let horizontal_count = (bounds_width_plus_padding / min_element_width_plus_padding).floor().max(1.0) as usize;

                            let (element_width, horizontal_count) = if horizontal_count >= children_count {
                                (min_element_width, children_count)
                            } else {
                                let padding_width = gap_width * (horizontal_count - 1);
                                let width = (base_width - padding_width) / horizontal_count as f32;

                                (width, horizontal_count)
                            };

                            (element_width * children_count + gap_width * children_count.saturating_sub(1), horizontal_count)
                        };

                        let rows = (children_count + horizontal_count - 1) / horizontal_count;
                        let height = (min_element_height + gap_height) * rows;

                        size(width, height)
                    },
                )
            },
        );

        (layout_id, Size::new(min_element_width, min_element_height))
    }

    fn prepaint(
        &mut self,
        global_id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        element_size: &mut Self::RequestLayoutState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |style, _scroll_offset, _hitbox, window, cx| {
                if self.children.is_empty() {
                    return;
                }

                let rem_size = window.rem_size();
                let font_size = window.text_style().font_size;
                let gap_width = style.gap.width.to_pixels(font_size, rem_size);
                let gap_height = style.gap.height.to_pixels(font_size, rem_size);
                let children_count = self.children.len();

                let bounds_width_plus_padding = bounds.size.width.to_f64() + gap_width.to_f64();
                let min_element_width_plus_padding = element_size.width.to_f64() + gap_width.to_f64();
                let horizontal_count = (bounds_width_plus_padding / min_element_width_plus_padding).floor().max(1.0) as usize;

                let (mut width, horizontal_count) = if horizontal_count >= children_count {
                    (element_size.width, children_count)
                } else {
                    let padding_width = gap_width * (horizontal_count - 1);
                    let width = (bounds.size.width - padding_width) / horizontal_count as f32;

                    (width, horizontal_count)
                };

                for (index, child) in self.children.iter_mut().enumerate() {
                    let available_space = Size::new(
                        gpui::AvailableSpace::Definite(width),
                        self.min_element_size.height
                    );
                    child.layout_as_root(available_space, window, cx);
                    let h_index = index % horizontal_count;
                    let v_index = index / horizontal_count;
                    let offset = Point::new(
                        (width + gap_width) * h_index,
                        (element_size.height + gap_height) * v_index
                    );
                    child.prepaint_at(bounds.origin + offset, window, cx);
                }
            },
        );
    }

    fn paint(
        &mut self,
        global_id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            None,
            window,
            cx,
            |_style, window, cx| {
                for child in &mut self.children {
                    child.paint(window, cx);
                }
            },
        )
    }
}
