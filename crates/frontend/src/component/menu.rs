use std::rc::Rc;

use gpui::{App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder};
use gpui_component::{ActiveTheme, StyledExt, v_flex};

#[derive(IntoElement)]
pub struct MenuGroup {
    title: SharedString,
    children: Vec<MenuGroupItem>,
}

impl MenuGroup {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            children: Vec::new(),
        }
    }

    pub fn child(mut self, child: MenuGroupItem) -> Self {
        self.children.push(child);
        self
    }
}

impl RenderOnce for MenuGroup {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let title = div()
            .px_2()
            .pt_3()
            .text_xs()
            .font_bold()
            .text_color(cx.theme().sidebar_foreground.opacity(0.7))
            .child(self.title);

        v_flex().gap_1().child(title).children(self.children)
    }
}

#[derive(IntoElement)]
pub struct MenuGroupItem {
    title: SharedString,
    active: bool,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl MenuGroupItem {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            active: false,
            on_click: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MenuGroupItem {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let mut item = div()
            .id(self.title.clone())
            .px_2()
            .py_0p5()
            .text_sm()
            .child(self.title)
            .rounded(cx.theme().radius)
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| {
                    (on_click)(event, window, cx);
                })
            });

        if self.active {
            item = item.font_medium()
                .bg(cx.theme().sidebar_accent)
                .text_color(cx.theme().sidebar_accent_foreground);
        } else {
            item = item.hover(|this| {
                this.bg(cx.theme().sidebar_accent.opacity(0.8))
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
        }

        item
    }
}
