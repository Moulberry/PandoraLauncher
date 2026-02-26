use gpui::*;
use gpui_component::{ActiveTheme, h_flex, scroll::ScrollableElement, v_flex};

#[derive(IntoElement)]
pub struct Page {
    title: AnyElement,
    scrollable: bool,
    children: Vec<AnyElement>,
}

impl Page {
    pub fn new(title: impl IntoElement) -> Self {
        Self {
            title: title.into_any_element(),
            scrollable: false,
            children: Vec::new(),
        }
    }

    pub fn scrollable(mut self) -> Self {
        self.scrollable = true;
        self
    }
}

impl ParentElement for Page {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Page {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let title = h_flex()
            .w_full()
            .min_h(px(65.0))
            .max_h(px(65.0))
            .h(px(65.0))
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xl()
            .child(div().left_4().child(self.title));

        if self.scrollable {
            v_flex()
                .size_full()
                .child(title)
                .child(div().flex_1().overflow_hidden().child(
                    v_flex().size_full().overflow_y_scrollbar().children(self.children),
                ))
        } else {
            v_flex()
                .size_full()
                .child(title)
                .children(self.children)
        }
    }
}
