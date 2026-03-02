use gpui::{AnyElement, AvailableSpace, Bounds, ContentMask, Element, InteractiveElement, Interactivity, IntoElement, ParentElement, Pixels, Point, Size, StyleRefinement, Styled, px, size};
use gpui_component::ActiveTheme;

pub struct HorizontalSections {
    interactivity: Interactivity,
    children: Vec<AnyElement>,
}

impl HorizontalSections {
    pub fn new() -> Self {
        Self {
            interactivity: Interactivity::default(),
            children: Vec::new(),
        }
    }
}

impl Styled for HorizontalSections {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for HorizontalSections {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl ParentElement for HorizontalSections {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl IntoElement for HorizontalSections {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for HorizontalSections {
    type RequestLayoutState = ();
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
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| {
                window.request_layout(style, [], cx)
            },
        );

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
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
                let children_count = self.children.len();
                let padding = style.padding.to_pixels(bounds.size.into(), rem_size);

                let bounds = Bounds {
                    origin: bounds.origin.clone() + gpui::point(padding.left.clone(), padding.top.clone()),
                    size: (bounds.size.clone() - size(
                            padding.left.clone() + padding.right.clone(),
                            padding.top.clone() + padding.bottom,
                        )).max(&Default::default()),
                };

                let total_gap_width = gap_width * (children_count - 1);
                let available_space_for_children = bounds.size.width - total_gap_width;
                if available_space_for_children < Pixels::ZERO {
                    for child in self.children.iter_mut() {
                        let available_space = Size::new(AvailableSpace::Definite(Pixels::ZERO), AvailableSpace::MinContent);
                        child.layout_as_root(available_space, window, cx);
                        child.prepaint_at(bounds.origin, window, cx);
                    }
                } else {
                    let width = available_space_for_children / children_count as f32;
                    let offset = width + gap_width;

                    for (index, child) in self.children.iter_mut().enumerate() {
                        let available_space = Size::new(AvailableSpace::Definite(width), AvailableSpace::MinContent);
                        child.layout_as_root(available_space, window, cx);
                        let mut origin = bounds.origin;
                        origin.x += offset * index;
                        child.prepaint_at(origin, window, cx);
                    }
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
            |style, window, cx| {
                let rem_size = window.rem_size();
                let font_size = window.text_style().font_size;
                let gap_width = style.gap.width.to_pixels(font_size, rem_size);
                let children_count = self.children.len();
                let padding = style.padding.to_pixels(bounds.size.into(), rem_size);

                let original_bounds = bounds;
                let bounds = Bounds {
                    origin: bounds.origin.clone() + gpui::point(padding.left.clone(), padding.top.clone()),
                    size: (bounds.size.clone() - size(
                            padding.left.clone() + padding.right.clone(),
                            padding.top.clone() + padding.bottom,
                        )).max(&Default::default()),
                };

                let total_gap_width = gap_width * (children_count - 1);
                let available_space_for_children = bounds.size.width - total_gap_width;

                if available_space_for_children > Pixels::ZERO {
                    let width = available_space_for_children / children_count as f32;
                    let offset = width + gap_width;

                    for index in 1..children_count {
                        let mut origin = bounds.origin;
                        origin.x += offset * index;
                        origin.x -= gap_width / 2.0;
                        let fill_bounds = Bounds {
                            origin,
                            size: Size::new(px(1.0), bounds.size.height),
                        };
                        window.paint_quad(gpui::fill(fill_bounds, cx.theme().border));
                    }

                    let mut x = original_bounds.origin.x;
                    for (index, child) in self.children.iter_mut().enumerate() {
                        let mut full_width = width;

                        if index == 0 {
                            full_width += padding.left + gap_width / 2.0;
                        } else if index == children_count-1 {
                            full_width += gap_width / 2.0 + padding.right;
                        } else {
                            full_width += gap_width;
                        }

                        let child_bounds = Bounds {
                            origin: Point::new(x, original_bounds.origin.y),
                            size: Size::new(width + gap_width - px(1.0), original_bounds.size.height)
                        };
                        window.with_content_mask(Some(ContentMask { bounds: child_bounds }), |window| {
                            child.paint(window, cx);
                        });

                        x += full_width;
                    }
                }
            },
        )
    }
}
