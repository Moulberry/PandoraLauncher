use std::sync::Arc;

use gpui::SharedString;
use gpui_component::breadcrumb::{Breadcrumb, BreadcrumbItem};
use gpui::*;

use crate::{entity::{DataEntities, instance::InstanceEntries}, ui::PageType};

pub struct PagePath {
    pages: Arc<[PageType]>,
}

impl PagePath {
    pub fn new(pages: Arc<[PageType]>) -> Self {
        Self { pages }
    }

    pub fn create_breadcrumb(&self, data: &DataEntities, cx: &App) -> Breadcrumb {
        let mut breadcrumb = Breadcrumb::new().text_xl();

        let pages = self.pages.clone();

        for i in 0..pages.len() {
            let title = match pages[i] {
                PageType::Instances => "Instances".into(),
                PageType::Syncing => "Syncing".into(),
                PageType::Modrinth { installing_for, .. } => {
                    if installing_for.is_some() {
                        "Add from Modrinth".into()
                    } else {
                        "Modrinth".into()
                    }
                },
                PageType::InstancePage(instance_id, _) => {
                    InstanceEntries::find_title_by_id(&data.instances, instance_id, cx)
                        .unwrap_or("<instance name>".into())
                },
            };

            let mut item = BreadcrumbItem::new(title);

            if i < pages.len()-1 {
                let pages = pages.clone();
                item = item.on_click(move |_, window, cx| {
                    let page = pages[i];
                    let rest = &pages[0..i];
                    crate::root::switch_page(page, rest.into(), window, cx);
                });
            }

            breadcrumb = breadcrumb.child(item);
        }

        breadcrumb
    }
}
