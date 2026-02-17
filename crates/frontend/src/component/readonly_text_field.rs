use std::{cell::RefCell, num::NonZeroUsize, ops::Range, rc::Rc, sync::Arc};

use ftree::FenwickTree;
use gpui::{prelude::*, *};
use gpui_component::{
    button::Button, h_flex, input::{Input, InputEvent, InputState}, scroll::{Scrollbar, ScrollbarHandle}, v_flex, ActiveTheme as _, Icon, IconName, Sizable
};
use lru::LruCache;
use rustc_hash::FxBuildHasher;

use crate::ts;

struct CachedShapedLines {
    item_lines: LruCache<usize, WrappedLines, FxBuildHasher>,
}

struct ReadonlyTextFieldItemState {
    items: Vec<TextFieldLine>,
    last_scrolled_item: usize,
    item_sizes: FenwickTree<usize>,
    total_line_count: usize,
    cached_shaped_lines: CachedShapedLines,
    search_query: SharedString,
}

pub struct ReadonlyTextField {
    font: Font,
    scroll_state: Rc<RefCell<GameOutputScrollState>>,
    pending: Vec<Arc<str>>,
    item_state: Option<ReadonlyTextFieldItemState>,
}

impl Default for ReadonlyTextField {
    fn default() -> Self {
        Self {
            font: Font {
                family: SharedString::new_static("Roboto Mono"),
                features: FontFeatures::default(),
                fallbacks: None,
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            },
            scroll_state: Default::default(),
            pending: Vec::new(),
            item_state: Some(ReadonlyTextFieldItemState {
                items: Vec::new(),
                last_scrolled_item: 0,
                item_sizes: FenwickTree::new(),
                total_line_count: 0,
                cached_shaped_lines: CachedShapedLines {
                    item_lines: LruCache::with_hasher(NonZeroUsize::new(256).unwrap(), FxBuildHasher),
                },
                search_query: SharedString::new_static(""),
            }),
        }
    }
}

impl ReadonlyTextField {
    pub fn add(&mut self, line: Arc<str>) {
        self.pending.push(line);
    }

    pub fn shrink_to_fit(&mut self) {
        self.pending.shrink_to_fit();
        if let Some(item_state) = self.item_state.as_mut() {
            item_state.items.shrink_to_fit();
        }
    }

    pub fn apply_pending(&mut self) {
        let Some(item_state) = &mut self.item_state else {
            return;
        };
        for line in self.pending.drain(..) {
            let mut highlighted_text = None;

            if !item_state.search_query.is_empty() {
                if let Some(found) = line.find(item_state.search_query.as_str()) {
                    highlighted_text = Some(found..found+item_state.search_query.as_str().len());
                }
                if highlighted_text.is_none() {
                    // Item doesn't match search query, push skipped item
                    item_state.item_sizes.push(0);
                    item_state.items.push(TextFieldLine {
                        line: line.clone(),
                        index: item_state.items.len(),
                        backup_total_lines_while_skipped: 1,
                        total_lines: 0,
                        highlighted_text: None,
                        skip: true,
                    });
                    continue;
                }
            }

            item_state.item_sizes.push(1);
            item_state.total_line_count += 1;
            item_state.items.push(TextFieldLine {
                line: line.clone(),
                index: item_state.items.len(),
                backup_total_lines_while_skipped: 1,
                total_lines: 1,
                highlighted_text,
                skip: false,
            });
        }
    }
}

pub struct ReadonlyTextFieldComponent {
    interactivity: Interactivity,
    text_field: Entity<ReadonlyTextField>,
}

struct TextFieldLine {
    line: Arc<str>,
    index: usize,
    backup_total_lines_while_skipped: usize,
    total_lines: usize,
    highlighted_text: Option<Range<usize>>,
    skip: bool,
}

impl TextFieldLine {
    pub fn compute_wrapped_text<'a>(
        &mut self,
        wrap_width: Pixels,
        text_system: &Arc<WindowTextSystem>,
        font: &Font,
        font_size: Pixels,
        text_style: &TextStyle,
        line_wrapper: &mut LineWrapperHandle,
        cache: &'a mut CachedShapedLines,
    ) -> &'a [ShapedLine] {
        let mut recompute = true;

        if let Some(last_wrapped) = cache.item_lines.get(&self.index)
            && (last_wrapped.wrap_width == wrap_width || (last_wrapped.lines.len() == 1 && last_wrapped.lines.first().unwrap().width < wrap_width)) {
                recompute = false;
            }

        if recompute {
            let line = &self.line;
            let mut wrapped = Vec::new();
            let fragments = [LineFragment::Text { text: line }];
            let boundaries = line_wrapper.wrap_line(&fragments, wrap_width);

            let mut handle_segment = |wrapped_line: SharedString, from, to| {
                let runs: &[TextRun] = if let Some(highlight_range) = &self.highlighted_text
                    && highlight_range.start < to
                    && highlight_range.end > from
                {
                    let highlight_start = highlight_range.start.max(from);
                    let highlight_end = highlight_range.end.min(to);

                    &[
                        TextRun {
                            len: highlight_start - from,
                            font: font.clone(),
                            color: text_style.color,
                            background_color: text_style.background_color,
                            underline: text_style.underline,
                            strikethrough: text_style.strikethrough,
                        },
                        TextRun {
                            len: highlight_end - highlight_start,
                            font: font.clone(),
                            color: gpui::black(),
                            background_color: Some(gpui::yellow()),
                            underline: text_style.underline,
                            strikethrough: text_style.strikethrough,
                        },
                        TextRun {
                            len: to - highlight_end,
                            font: font.clone(),
                            color: text_style.color,
                            background_color: text_style.background_color,
                            underline: text_style.underline,
                            strikethrough: text_style.strikethrough,
                        },
                    ]
                } else {
                    &[TextRun {
                        len: wrapped_line.len(),
                        font: font.clone(),
                        color: text_style.color,
                        background_color: text_style.background_color,
                        underline: text_style.underline,
                        strikethrough: text_style.strikethrough,
                    }]
                };

                let shaped = text_system.shape_line(wrapped_line, font_size, runs, None);
                wrapped.push(shaped);
            };

            let mut last_boundary_ix = 0;
            for boundary in boundaries {
                let wrapped_line = &line[last_boundary_ix..boundary.ix];
                let wrapped_line = SharedString::new(wrapped_line);
                (handle_segment)(wrapped_line, last_boundary_ix, boundary.ix);
                last_boundary_ix = boundary.ix;
            }

            // Push last segment
            let wrapped_line = if last_boundary_ix == 0 {
                line.into()
            } else {
                SharedString::new(&line[last_boundary_ix..])
            };
            (handle_segment)(wrapped_line, last_boundary_ix, line.len());

            cache.item_lines.put(
                self.index,
                WrappedLines {
                    wrap_width,
                    lines: wrapped,
                },
            );
        }

        cache.item_lines.get(&self.index).unwrap().lines.as_slice()
    }
}

struct WrappedLines {
    wrap_width: Pixels,
    lines: Vec<ShapedLine>,
}

impl InteractiveElement for ReadonlyTextFieldComponent {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl IntoElement for ReadonlyTextFieldComponent {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ReadonlyTextFieldComponent {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.interactivity.request_layout(global_id, inspector_id, window, cx, |mut style, window, cx| {
            style.size.width = relative(1.0).into();
            style.size.height = relative(1.0).into();
            window.request_layout(style, None, cx)
        });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, _, _, _| {}
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.interactivity.paint(
                global_id,
                inspector_id,
                bounds,
                None,
                window,
                cx,
                |_, window, cx| {
                    let visible_bounds = bounds;
                    let mut bounds = bounds.inset(px(12.0));
                    bounds.size.width += px(12.0);

                    cx.update_entity(&self.text_field, |inner, cx| {
                        inner.apply_pending();

                        let text_style = window.text_style();

                        let font_size = text_style.font_size.to_pixels(window.rem_size());
                        let line_height = font_size * 1.25;

                        let text_width = bounds.size.width;
                        let wrap_width = text_width.max(font_size * 30);

                        let mut line_wrapper = window.text_system().line_wrapper(inner.font.clone(), font_size);

                        let scroll_render_info = inner.update_scrolling(line_height, wrap_width,
                            font_size, &text_style, &mut line_wrapper, window.text_system());

                        if let Some(item_state) = inner.item_state.as_mut() && !item_state.items.is_empty() {
                            if scroll_render_info.reverse {
                                paint_lines::<true>(
                                    item_state.items[..scroll_render_info.item+1].iter_mut().rev(),
                                    visible_bounds,
                                    bounds,
                                    scroll_render_info.offset,
                                    &inner.font,
                                    &text_style,
                                    wrap_width,
                                    font_size,
                                    line_height,
                                    &mut item_state.item_sizes,
                                    &mut item_state.total_line_count,
                                    &mut line_wrapper,
                                    &mut item_state.cached_shaped_lines,
                                    window,
                                    cx,
                                );
                            } else {
                                paint_lines::<false>(
                                    item_state.items[scroll_render_info.item..].iter_mut(),
                                    visible_bounds,
                                    bounds,
                                    scroll_render_info.offset,
                                    &inner.font,
                                    &text_style,
                                    wrap_width,
                                    font_size,
                                    line_height,
                                    &mut item_state.item_sizes,
                                    &mut item_state.total_line_count,
                                    &mut line_wrapper,
                                    &mut item_state.cached_shaped_lines,
                                    window,
                                    cx,
                                );
                            }
                        }

                        let mut scroll_state = inner.scroll_state.borrow_mut();
                        scroll_state.bounds_y = bounds.size.height;
                        scroll_state.line_height = line_height;
                        scroll_state.lines = if let Some(item_state) = &inner.item_state {
                            item_state.total_line_count
                        } else {
                            0
                        };
                    });
                });
        });
    }
}

#[derive(Debug)]
struct ScrollRenderInfo {
    item: usize,
    reverse: bool,
    offset: Pixels,
}

impl ReadonlyTextField {
    fn update_scrolling(
        &mut self,
        line_height: Pixels,
        wrap_width: Pixels,
        font_size: Pixels,
        text_style: &TextStyle,
        line_wrapper: &mut LineWrapperHandle,
        text_system: &Arc<WindowTextSystem>,
    ) -> ScrollRenderInfo {
        let mut scroll_state = self.scroll_state.borrow_mut();

        let Some(item_state) = self.item_state.as_mut() else {
            scroll_state.scrolling = GameOutputScrolling::Bottom;
            return ScrollRenderInfo {
                item: 0,
                reverse: true,
                offset: Pixels::ZERO,
            };
        };

        if item_state.items.is_empty() {
            scroll_state.scrolling = GameOutputScrolling::Bottom;
            item_state.last_scrolled_item = 0;
            return ScrollRenderInfo {
                item: 0,
                reverse: false,
                offset: Pixels::ZERO,
            };
        }

        let max_offset = (item_state.total_line_count * line_height - scroll_state.bounds_y).max(px(1.0));

        match &mut scroll_state.scrolling {
            GameOutputScrolling::Bottom => {
                if let Some(active_drag) = &mut scroll_state.active_drag {
                    active_drag.actual_offset = -max_offset;
                }
                item_state.last_scrolled_item = item_state.items.len().saturating_sub(1);
                ScrollRenderInfo {
                    item: item_state.items.len().saturating_sub(1),
                    reverse: true,
                    offset: Pixels::ZERO,
                }
            },
            GameOutputScrolling::Top { offset } => {
                let mut offset = *offset;

                for check_scrolled_items in [true, false] {
                    let mut effective_offset = offset;

                    if offset <= -max_offset {
                        scroll_state.scrolling = GameOutputScrolling::Bottom;
                        if let Some(active_drag) = &mut scroll_state.active_drag {
                            active_drag.actual_offset = -max_offset;
                        }
                        item_state.last_scrolled_item = item_state.items.len().saturating_sub(1);
                        return ScrollRenderInfo {
                            item: item_state.items.len().saturating_sub(1),
                            reverse: true,
                            offset: Pixels::ZERO,
                        };
                    }

                    if offset < px(-1.0)
                        && let Some(active_drag) = &scroll_state.active_drag
                    {
                        let drag_pivot = active_drag.drag_pivot.min(Pixels::ZERO);
                        let real_pivot = active_drag.real_pivot.min(Pixels::ZERO);
                        let new_max_offset =
                            (item_state.total_line_count * line_height - scroll_state.bounds_y).max(px(1.0));
                        let old_max_offset = (active_drag.start_content_height - scroll_state.bounds_y).max(px(1.0));

                        if offset < drag_pivot {
                            effective_offset = (offset - drag_pivot) / (-old_max_offset - drag_pivot)
                                * (-new_max_offset - real_pivot)
                                + real_pivot;
                        } else {
                            effective_offset = offset / drag_pivot * real_pivot;
                        }
                    }

                    if let Some(active_drag) = &mut scroll_state.active_drag {
                        active_drag.actual_offset = effective_offset;
                    }

                    let top = (-effective_offset).max(Pixels::ZERO);
                    let top_offset_for_inset = line_height.min(top);
                    let top = top - top_offset_for_inset;

                    let top_line = (top / line_height) as usize;
                    let line_remainder = top_line * line_height - top;

                    let (item_index, remainder_lines) = item_state.item_sizes.index_of_with_remainder(top_line + 1);

                    if check_scrolled_items && item_index < item_state.last_scrolled_item {
                        let mut resized_above = Pixels::ZERO;
                        let mut changed = false;
                        let from = item_index.max(item_state.last_scrolled_item.saturating_sub(32));
                        for item in item_state.items[from..item_state.last_scrolled_item].iter_mut() {
                            if item.skip {
                                continue;
                            }
                            let lines = item.compute_wrapped_text(
                                wrap_width,
                                text_system,
                                &self.font,
                                font_size,
                                text_style,
                                line_wrapper,
                                &mut item_state.cached_shaped_lines,
                            );
                            let line_count = lines.len().max(1);
                            if line_count != item.total_lines {
                                resized_above += line_count * line_height - item.total_lines * line_height;
                                if item.total_lines < line_count {
                                    item_state.item_sizes.add_at(item.index, line_count - item.total_lines);
                                    item_state.total_line_count += line_count - item.total_lines;
                                } else {
                                    item_state.item_sizes.sub_at(item.index, item.total_lines - line_count);
                                    item_state.total_line_count -= item.total_lines - line_count;
                                }
                                item.total_lines = line_count;
                                changed = true;
                            }
                        }
                        if changed {
                            if let Some(active_drag) = &mut scroll_state.active_drag {
                                active_drag.drag_pivot = offset;
                                active_drag.real_pivot = effective_offset - resized_above;
                            } else {
                                offset -= resized_above;
                                if let GameOutputScrolling::Top { offset } = &mut scroll_state.scrolling {
                                    *offset -= resized_above;
                                }
                            }
                            continue;
                        }
                    }

                    let render_offset = -(remainder_lines * line_height) + line_remainder + line_height - top_offset_for_inset;

                    if scroll_state.active_drag.is_some() {
                        let mut remaining_lines = ((scroll_state.bounds_y - render_offset) / line_height) as usize + 1;
                        let mut changed = false;
                        for item in item_state.items[item_index..].iter_mut() {
                            if item.skip {
                                continue;
                            }
                            let lines = item.compute_wrapped_text(
                                wrap_width,
                                text_system,
                                &self.font,
                                font_size,
                                text_style,
                                line_wrapper,
                                &mut item_state.cached_shaped_lines,
                            );
                            let line_count = lines.len().max(1);
                            if line_count != item.total_lines {
                                if item.total_lines < line_count {
                                    item_state.item_sizes.add_at(item.index, line_count - item.total_lines);
                                    item_state.total_line_count += line_count - item.total_lines;
                                } else {
                                    item_state.item_sizes.sub_at(item.index, item.total_lines - line_count);
                                    item_state.total_line_count -= item.total_lines - line_count;
                                }
                                item.total_lines = line_count;
                                changed = true;
                            }
                            remaining_lines = remaining_lines.saturating_sub(line_count);
                            if remaining_lines == 0 {
                                break;
                            }
                        }
                        if changed && let Some(active_drag) = &mut scroll_state.active_drag {
                            active_drag.drag_pivot = offset;
                            active_drag.real_pivot = effective_offset;
                        }
                    }

                    item_state.last_scrolled_item = item_index;
                    return ScrollRenderInfo {
                        item: item_index,
                        reverse: false,
                        offset: render_offset,
                    };
                }
                unreachable!();
            },
        }
    }
}

fn paint_lines<'a, const REVERSE: bool>(
    items: impl Iterator<Item = &'a mut TextFieldLine>,
    visible_bounds: Bounds<Pixels>,
    bounds: Bounds<Pixels>,
    offset: Pixels,
    font: &Font,
    text_style: &TextStyle,
    wrap_width: Pixels,
    font_size: Pixels,
    line_height: Pixels,
    item_sizes: &mut FenwickTree<usize>,
    total_line_count: &mut usize,
    line_wrapper: &mut LineWrapperHandle,
    cache: &mut CachedShapedLines,
    window: &mut Window,
    cx: &mut App,
) {
    let mut text_origin = bounds.origin;
    if REVERSE {
        text_origin.y += bounds.size.height;
        text_origin.y -= line_height;
    }
    text_origin.y += offset;

    for item in items {
        if item.skip {
            continue;
        }
        let has_highlighted_text = item.highlighted_text.is_some();

        let lines = item.compute_wrapped_text(
            wrap_width,
            window.text_system(),
            font,
            font_size,
            text_style,
            line_wrapper,
            cache,
        );

        let line_count = lines.len().max(1);

        let mut line_origin = text_origin;
        if REVERSE {
            for shaped in lines.iter().rev() {
                if line_origin.y <= visible_bounds.origin.y + visible_bounds.size.height {
                    if has_highlighted_text {
                        _ = shaped.paint_background(line_origin, line_height, TextAlign::Left, None, window, cx);
                    }
                    _ = shaped.paint(line_origin, line_height, TextAlign::Left, None, window, cx);
                }
                line_origin.y -= line_height;
            }
        } else {
            for shaped in lines.iter() {
                if line_origin.y >= visible_bounds.origin.y - line_height {
                    if has_highlighted_text {
                        _ = shaped.paint_background(line_origin, line_height, TextAlign::Left, None, window, cx);
                    }
                    _ = shaped.paint(line_origin, line_height, TextAlign::Left, None, window, cx);
                }
                line_origin.y += line_height;
            }
        }

        if line_count != item.total_lines {
            if item.total_lines < line_count {
                item_sizes.add_at(item.index, line_count - item.total_lines);
                *total_line_count += line_count - item.total_lines;
            } else {
                item_sizes.sub_at(item.index, item.total_lines - line_count);
                *total_line_count -= item.total_lines - line_count;
            }
            item.total_lines = line_count;
        }

        if REVERSE {
            text_origin.y -= line_count * line_height;
            if text_origin.y < visible_bounds.origin.y - line_height {
                break;
            }
        } else {
            text_origin.y += line_count * line_height;
            if text_origin.y > visible_bounds.origin.y + visible_bounds.size.height {
                break;
            }
        }
    }
}

pub struct ReadonlyTextFieldWithControls {
    scroll_handler: ScrollHandler,
    text_field: Entity<ReadonlyTextField>,
    search_state: Entity<InputState>,
    extra: Box<dyn Fn(Div) -> Div>,
    _search_task: Task<()>,
    _search_input_subscription: Subscription,
}

#[derive(Clone)]
pub struct ScrollHandler {
    state: Rc<RefCell<GameOutputScrollState>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct ActiveDrag {
    start_content_height: Pixels,
    drag_pivot: Pixels,
    real_pivot: Pixels,
    actual_offset: Pixels,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct GameOutputScrollState {
    lines: usize,
    line_height: Pixels,
    bounds_y: Pixels,
    scrolling: GameOutputScrolling,
    active_drag: Option<ActiveDrag>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum GameOutputScrolling {
    #[default]
    Bottom,
    Top {
        offset: Pixels,
    },
}

impl GameOutputScrollState {
    pub fn content_height_for_scrollbar(&self) -> Pixels {
        self.active_drag
            .as_ref()
            .map(|v| v.start_content_height)
            .unwrap_or(self.lines * self.line_height)
    }

    pub fn max_scroll_amount(&self) -> Pixels {
        (self.lines * self.line_height - self.bounds_y).max(Pixels::ZERO)
    }

    pub fn offset(&self) -> Pixels {
        match self.scrolling {
            GameOutputScrolling::Bottom => {
                let content_height = self.content_height_for_scrollbar();
                -(content_height - self.bounds_y)
            },
            GameOutputScrolling::Top { offset } => offset,
        }
    }

    pub fn set_offset(&mut self, new_offset: Pixels) {
        let content_height = self.content_height_for_scrollbar();
        let new_offset = new_offset.min(Pixels::ZERO);
        let total_offset = -(content_height - self.bounds_y);

        if new_offset < total_offset + self.line_height / 4.0 {
            self.scrolling = GameOutputScrolling::Bottom;
        } else {
            self.scrolling = GameOutputScrolling::Top { offset: new_offset };
        }
    }
}

impl ScrollbarHandle for ScrollHandler {
    fn offset(&self) -> Point<Pixels> {
        let state = self.state.borrow();
        Point::new(Pixels::ZERO, state.offset())
    }

    fn set_offset(&self, new_offset: Point<Pixels>) {
        let mut state = self.state.borrow_mut();
        state.set_offset(new_offset.y);
    }

    fn content_size(&self) -> Size<Pixels> {
        let state = self.state.borrow();
        let content_height = state.content_height_for_scrollbar();
        Size::new(Pixels::ZERO, content_height)
    }

    fn start_drag(&self) {
        let mut state = self.state.borrow_mut();
        state.active_drag = Some(ActiveDrag {
            start_content_height: state.lines * state.line_height,
            drag_pivot: Pixels::ZERO,
            real_pivot: Pixels::ZERO,
            actual_offset: state.offset(),
        });
    }

    fn end_drag(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(drag) = state.active_drag.take() {
            state.set_offset(drag.actual_offset);
        }
    }
}

impl ReadonlyTextFieldWithControls {
    pub fn new(
        text_field: Entity<ReadonlyTextField>,
        extra: Box<dyn Fn(Div) -> Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let scroll_state = Rc::clone(&text_field.read(cx).scroll_state);

        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder(ts!("common.search")).clean_on_escape());

        let _search_input_subscription = cx.subscribe_in(&search_state, window, Self::on_search_input_event);

        Self {
            scroll_handler: ScrollHandler { state: scroll_state },
            text_field,
            search_state,
            extra,
            _search_task: Task::ready(()),
            _search_input_subscription,
        }
    }

    fn on_search_input_event(
        &mut self,
        state: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let InputEvent::PressEnter { secondary: false } = event else {
            return;
        };

        let item_state = self.text_field.update(cx, |game_output, _| game_output.item_state.take());

        let Some(mut item_state) = item_state else {
            return; // Already searching
        };

        let search_pattern = state.read(cx).value();
        if search_pattern.trim().is_empty() {
            self._search_task = cx.spawn_in(window, async move |this, window| {
                let mut lengths = Vec::new();
                item_state.total_line_count = 0;
                for item in &mut item_state.items {
                    if item.skip {
                        item.total_lines = item.backup_total_lines_while_skipped;
                    }

                    item.skip = false;
                    item.highlighted_text = None;

                    item_state.total_line_count += item.total_lines;
                    lengths.push(item.total_lines);
                }
                item_state.item_sizes = FenwickTree::from_iter(lengths.into_iter());
                item_state.cached_shaped_lines.item_lines.clear();
                item_state.search_query = SharedString::new_static("");

                this.update_in(window, |this, window, cx| {
                    this.text_field.update(cx, |text_field, _| {
                        text_field.item_state = Some(item_state);
                    });
                    this.search_state.update(cx, |input, cx| input.set_loading(false, window, cx));
                    cx.notify();
                }).unwrap();
            });
        } else {
            self._search_task = cx.spawn_in(window, async move |this, window| {
                let mut lengths = Vec::new();
                item_state.total_line_count = 0;
                for item in &mut item_state.items {
                    let mut contains = None;
                    if let Some(found) = item.line.find(search_pattern.as_str()) {
                        contains = Some(found..found+search_pattern.as_str().len());
                    }
                    if contains.is_some() {
                        lengths.push(item.total_lines);
                        item_state.total_line_count += item.total_lines;

                        item.highlighted_text = contains;
                        item.skip = false;
                    } else {
                        item.backup_total_lines_while_skipped = item.total_lines;
                        item.total_lines = 0;
                        lengths.push(0);

                        item.skip = true;
                    }
                }
                item_state.item_sizes = FenwickTree::from_iter(lengths.into_iter());
                item_state.cached_shaped_lines.item_lines.clear();
                item_state.search_query = search_pattern;

                this.update_in(window, |this, window, cx| {
                    this.text_field.update(cx, |text_field, _| {
                        text_field.item_state = Some(item_state);
                    });
                    this.search_state.update(cx, |input, cx| input.set_loading(false, window, cx));
                    cx.notify();
                })
                .unwrap();
            });
        }

        state.update(cx, |input, cx| input.set_loading(true, window, cx));
    }
}

impl Render for ReadonlyTextFieldWithControls {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search = Input::new(&self.search_state).prefix(Icon::new(IconName::Search).small());

        let bar = h_flex()
            .w_full()
            .rounded(cx.theme().radius)
            .flex_1()
            .gap_4()
            .child(search)
            .child(Button::new("top").label(ts!("common.nav.top")).on_click(cx.listener(|root, _, _, cx| {
                let mut state = root.scroll_handler.state.borrow_mut();
                state.scrolling = GameOutputScrolling::Top { offset: Pixels::ZERO };
                cx.notify();
            })))
            .child(Button::new("bottom").label(ts!("common.nav.bottom")).on_click(cx.listener(|root, _, _, cx| {
                let mut state = root.scroll_handler.state.borrow_mut();
                state.scrolling = GameOutputScrolling::Bottom;
                cx.notify();
            })));

        let bar = (self.extra)(bar);

        v_flex()
            .size_full()
            .border_12()
            .gap_4()
            .child(bar)
            .child(
                h_flex()
                    .size_full()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(ReadonlyTextFieldComponent {
                        interactivity: Interactivity::new(),
                        text_field: self.text_field.clone(),
                    })
                    .child(
                        div()
                            .w_3()
                            .h_full()
                            .border_y_12()
                            .child(Scrollbar::vertical(&self.scroll_handler)),
                    ),
            )
            .on_scroll_wheel(cx.listener(|root, event: &ScrollWheelEvent, _, cx| {
                let state = root.scroll_handler.state.borrow();
                let delta = event.delta.pixel_delta(state.line_height).y;
                let max_scroll_amount = state.max_scroll_amount();
                drop(state);

                let current_offset = root.scroll_handler.offset().y;
                let new_offset = (current_offset + delta).clamp(-max_scroll_amount, Pixels::ZERO);
                if current_offset != new_offset {
                    root.scroll_handler.set_offset(Point::new(Pixels::ZERO, new_offset));
                    cx.notify();
                }
            }))
    }
}
