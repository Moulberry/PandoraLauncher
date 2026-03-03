use std::sync::Arc;

use gpui_component::{ActiveTheme, Icon, h_flex};
use gpui::*;

use crate::{icon::PandoraIcon, ui::PageType};

#[derive(IntoElement)]
pub struct PagePath {
    main_page: PageType,
    breadcrumb: Arc<[PageType]>,
}

impl PagePath {
    pub fn new(main_page: PageType, breadcrumb: Arc<[PageType]>) -> Self {
        Self { main_page, breadcrumb }
    }
}

impl RenderOnce for PagePath {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_1p5()
            .text_xl()
            .text_color(cx.theme().muted_foreground)
            .children(self.breadcrumb.iter().enumerate().flat_map(|(i, page)| {
                let item = div()
                    .id(i)
                    .child(page.title())
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, window, _| {
                        window.prevent_default();
                    })
                    .on_click({
                        let pages = self.breadcrumb.clone();
                        move |_, window, cx| {
                            let page = pages[i].clone();
                            let rest = &pages[0..i];
                            crate::root::switch_page(page, rest, window, cx);
                        }
                    }).into_any_element();
                [
                    item,
                    Icon::new(PandoraIcon::ChevronRight).size_3p5().into_any_element()
                ]
            }))
            .child(div().text_color(cx.theme().foreground).child(self.main_page.title()))
    }
}
