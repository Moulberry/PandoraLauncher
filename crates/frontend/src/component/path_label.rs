use std::{cell::RefCell, ops::Range, path::{Component, Path}, rc::Rc, sync::{Arc, atomic::AtomicU32}};

use gpui::{AvailableSpace, Element, ElementId, IntoElement, ParentElement, ShapedLine, SharedString, Size, Style, TextStyle, px};
use gpui_component::button::{Button, ButtonVariants};

use crate::{icon::PandoraIcon, ts};

#[derive(Clone)]
pub struct PathLabel {
    state: Rc<RefCell<PathLabelState>>,
}

impl PathLabel {
    pub fn new(path: impl Into<Arc<Path>>, is_folder: bool) -> Self {
        Self {
            state: Rc::new(RefCell::new(PathLabelState::new(path.into(), is_folder)))
        }
    }

    pub fn path(&self) -> Arc<Path> {
        self.state.borrow().path.clone()
    }

    pub fn button(&self, id: impl Into<ElementId>) -> Button {
        let state = self.state.borrow();
        let icon = if state.is_folder {
            PandoraIcon::Folder
        } else {
            PandoraIcon::File
        };
        Button::new(id).success().icon(icon).child(self.clone()).tooltip(state.lossy_path_name.clone())
    }

    pub fn button_opt(label: &Option<Self>, id: impl Into<ElementId>) -> Button {
        if let Some(label) = label {
            label.button(id)
        } else {
            Button::new(id).success().icon(PandoraIcon::File).label(ts!("common.unset"))
        }
    }
}

struct PathFragment {
    text: SharedString,
    shaped: Option<ShapedLine>,
    needs_divider: bool,
    can_truncate: bool,
}

#[derive(Debug)]
struct TruncationInfo {
    ignored_range: Option<Range<usize>>,
    total_width: f32,
}

struct PathLabelState {
    path: Arc<Path>,
    lossy_path_name: SharedString,
    is_folder: bool,
    fragments: Vec<PathFragment>,
    full_width: f32,
    last_text_style: Option<TextStyle>,
    shaped_divider: Option<ShapedLine>,
    shaped_ellipsis: Option<ShapedLine>,
    min_truncation_info: Option<TruncationInfo>,
    last_truncation_info: Option<(f32, TruncationInfo)>
}

impl PathLabelState {
    fn new(path: Arc<Path>, is_folder: bool) -> Self {
        let mut fragments: Vec<PathFragment> = path.components().map(|comp| {
            if matches!(comp, Component::RootDir) {
                PathFragment {
                    text: SharedString::new_static("/"),
                    shaped: None,
                    needs_divider: false,
                    can_truncate: false,
                }
            } else {
                PathFragment {
                    text: SharedString::new(comp.as_os_str().to_string_lossy()),
                    shaped: None,
                    needs_divider: true,
                    can_truncate: !matches!(comp, Component::Prefix(_))
                }
            }
        }).collect();

        if let Some(last_fragment) = fragments.last_mut() {
            last_fragment.needs_divider &= is_folder;
            last_fragment.can_truncate = false;
        }

        Self {
            lossy_path_name: SharedString::new(path.to_string_lossy()),
            path,
            is_folder,
            fragments,
            full_width: 0.0,
            last_text_style: None,
            shaped_divider: None,
            shaped_ellipsis: None,
            min_truncation_info: None,
            last_truncation_info: None,
        }
    }

    fn compute_truncation_cached(&mut self, available: f32) -> &TruncationInfo {
        if self.last_truncation_info.is_none() {
            self.last_truncation_info = Some((available, self.compute_truncation(available)));
        } else {
            let (last_available, last_info) = self.last_truncation_info.as_ref().unwrap();
            if available < last_info.total_width || available > *last_available {
                self.last_truncation_info = Some((available, self.compute_truncation(available)));
            }
        }

        &self.last_truncation_info.as_mut().unwrap().1
    }

    fn compute_truncation(&self, available: f32) -> TruncationInfo {
        if available >= self.full_width {
            TruncationInfo {
                ignored_range: None,
                total_width: self.full_width
            }
        } else {
            let divider_width = self.shaped_divider.as_ref().unwrap().width.as_f32();
            let ellipsis_width = self.shaped_ellipsis.as_ref().unwrap().width.as_f32();

            let mut remaining = self.full_width + divider_width + ellipsis_width;

            let mut start = self.fragments.len()/2;
            let mut end = self.fragments.len()/2;
            let mut can_left = true;
            let mut can_right = true;
            let mut left = true;

            loop {
                if (!can_left && !can_right) || remaining <= available {
                    return TruncationInfo {
                        ignored_range: if start == end {
                            None
                        } else {
                            Some(start..end)
                        },
                        total_width: remaining
                    };
                }
                if !can_left && left {
                    left = false;
                }
                if !can_right && !left {
                    left = true;
                }

                let mid = if left {
                    start.saturating_sub(1)
                } else {
                    end
                };

                let fragment = &self.fragments[mid];

                if !fragment.can_truncate {
                    if left {
                        can_left = false;
                    } else {
                        can_right = false;
                    }
                    continue;
                }

                if left {
                    start = start.saturating_sub(1);
                    if start == 0 {
                        can_left = false;
                    }
                } else {
                    end = end.saturating_add(1);
                }

                remaining -= fragment.shaped.as_ref().unwrap().width.as_f32();
                if fragment.needs_divider {
                    remaining -= divider_width;
                }
                left = !left;
            }
        }
    }
}

impl IntoElement for PathLabel {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PathLabel {
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
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = text_style.line_height.to_pixels(font_size.into(), window.rem_size());

        {
            let mut state = self.state.borrow_mut();
            if state.last_text_style.as_ref().map(|style| style != &text_style).unwrap_or(true) {
                let shaped_divider = window.text_system().shape_line(SharedString::new_static("/"), font_size,
                    &[text_style.to_run(1)], None);

                let shaped_ellipsis = window.text_system().shape_line(SharedString::new_static("…"), font_size,
                    &[text_style.to_run("…".len())], None);

                let mut full_width = 0.0;
                for fragment in state.fragments.iter_mut() {
                    let line = window.text_system().shape_line(fragment.text.clone(), font_size, &[text_style.to_run(fragment.text.len())], None);

                    full_width += line.width.as_f32();

                    if fragment.needs_divider {
                        full_width += shaped_divider.width.as_f32();
                    }

                    fragment.shaped = Some(line);
                }

                state.shaped_ellipsis = Some(shaped_ellipsis);
                state.shaped_divider = Some(shaped_divider);
                state.full_width = full_width;
                state.min_truncation_info = None;
                state.last_truncation_info = None;
                state.last_text_style = Some(text_style);
            }
        }

        let layout_id = window.request_measured_layout(Style::default(), {
            let state = self.state.clone();
            let last_definite_width = AtomicU32::new(0);
            move |known_dimensions, available_space, _window, _cx| {
                let mut state = state.borrow_mut();

                let width = if let Some(pixels) = known_dimensions.width {
                    last_definite_width.store(pixels.as_f32().ceil() as u32, std::sync::atomic::Ordering::Relaxed);
                    pixels.as_f32().ceil()
                } else {
                    match available_space.width {
                        AvailableSpace::Definite(pixels) => {
                            last_definite_width.store(pixels.as_f32().ceil() as u32, std::sync::atomic::Ordering::Relaxed);
                            pixels.as_f32().ceil()
                        },
                        AvailableSpace::MinContent => 0.0,
                        AvailableSpace::MaxContent => {
                            let last_definite_width = last_definite_width.load(std::sync::atomic::Ordering::Relaxed);
                            if last_definite_width > 0 {
                                last_definite_width as f32
                            } else {
                                state.full_width.ceil()
                            }
                        },
                    }
                };

                if width == 0.0 {
                    if let Some(min_truncation_info) = &state.min_truncation_info {
                        Size { width: px(min_truncation_info.total_width.ceil()), height: line_height }
                    } else {
                        let truncation = state.compute_truncation(0.0);
                        let width = truncation.total_width;
                        state.min_truncation_info = Some(truncation);
                        Size { width: px(width.ceil()), height: line_height }
                    }
                } else if width >= state.full_width {
                    Size { width: px(state.full_width.ceil()), height: line_height }
                } else {
                    let truncation = state.compute_truncation_cached(width);
                    Size { width: px(truncation.total_width.ceil()), height: line_height }
                }
            }
        });

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        ()
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
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = text_style.line_height.to_pixels(font_size.into(), window.rem_size());

        let mut state = self.state.borrow_mut();

        let truncation = state.compute_truncation_cached(bounds.size.width.as_f32().ceil());
        let skip_range = truncation.ignored_range.clone().unwrap_or(usize::MAX..usize::MAX);

        let divider = state.shaped_divider.as_ref().unwrap();

        let mut origin = bounds.origin;

        let mut index = 0;

        while index < state.fragments.len() {
            if index == skip_range.start {
                let ellipsis = state.shaped_ellipsis.as_ref().unwrap();
                _ = ellipsis.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
                origin.x += ellipsis.width;

                _ = divider.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
                origin.x += divider.width;

                index = skip_range.end;
            }

            let fragment = &state.fragments[index];

            let fragment_shaped = fragment.shaped.as_ref().unwrap();
            _ = fragment_shaped.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
            origin.x += fragment_shaped.width;

            if fragment.needs_divider {
                _ = divider.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
                origin.x += divider.width;
            }

            index += 1;
        }
    }
}
