use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme as _, StyledExt, h_flex, v_flex};

use crate::icon::PandoraIcon;

#[derive(IntoElement)]
pub struct ErrorAlert {
    id: ElementId,
    title: SharedString,
    message: SharedString,
}

impl ErrorAlert {
    pub fn new(id: impl Into<ElementId>, title: SharedString, message: SharedString) -> Self {
        Self {
            id: id.into(),
            title,
            message,
        }
    }
}

impl RenderOnce for ErrorAlert {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let radius = cx.theme().radius;
        let padding_x = px(16.0);
        let padding_y = px(10.0);
        let gap = px(12.0);

        let danger = cx.theme().danger;
        let bg = danger.opacity(0.08);
        let fg = cx.theme().red;
        let border_color = danger;

        h_flex()
            .id(self.id)
            .w_full()
            .text_color(fg)
            .bg(bg)
            .px(padding_x)
            .py(padding_y)
            .gap(gap)
            .justify_between()
            .text_sm()
            .border_1()
            .border_color(border_color)
            .rounded(radius)
            .items_start()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .gap(gap)
                    .child(div().mt(px(6.0)).child(PandoraIcon::CircleX))
                    .child(
                        v_flex()
                            .overflow_hidden()
                            .child(div().w_full().text_base().truncate().font_semibold().child(self.title))
                            .child(self.message),
                    ),
            )
            .into_any_element()
    }
}
