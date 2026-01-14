use std::sync::Arc;

use bridge::{instance::InstanceID, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{
    breadcrumb::{Breadcrumb, BreadcrumbItem}, button::{Button, ButtonVariants}, h_flex, resizable::{h_resizable, resizable_panel, ResizableState}, sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarMenu, SidebarMenuItem}, v_flex, ActiveTheme as _, Icon, IconName, WindowExt
};
use serde::{Deserialize, Serialize};

use crate::{
    entity::{
        instance::{InstanceAddedEvent, InstanceEntries, InstanceModifiedEvent, InstanceMovedToTopEvent, InstanceRemovedEvent}, DataEntities
    }, interface_config::InterfaceConfig, modals, pages::{instance::instance_page::{InstancePage, InstanceSubpageType}, instances_page::InstancesPage, modrinth_page::ModrinthSearchPage, syncing_page::SyncingPage}, png_render_cache, root
};

pub struct LauncherUI {
    data: DataEntities,
    page: LauncherPage,
    sidebar_state: Entity<ResizableState>,
    recent_instances: heapless::Vec<(InstanceID, SharedString), 3>,
    _instance_added_subscription: Subscription,
    _instance_modified_subscription: Subscription,
    _instance_removed_subscription: Subscription,
    _instance_moved_to_top_subscription: Subscription,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum PageType {
    Instances,
    Syncing,
    Modrinth {
        installing_for: Option<InstanceID>,
    },
    InstancePage(InstanceID, InstanceSubpageType),
}

impl PageType {
    fn to_serialized(&self, data: &DataEntities, cx: &App) -> SerializedPageType {
        match self {
            PageType::Instances => SerializedPageType::Instances,
            PageType::Syncing => SerializedPageType::Syncing,
            PageType::Modrinth { installing_for } => {
                if let Some(installing_for) = installing_for {
                    if let Some(name) = InstanceEntries::find_name_by_id(&data.instances, *installing_for, cx) {
                        return SerializedPageType::Modrinth { installing_for: Some(name) };
                    }
                }
                SerializedPageType::Modrinth { installing_for: None}
            },
            PageType::InstancePage(id, _) => {
                if let Some(name) = InstanceEntries::find_name_by_id(&data.instances, *id, cx) {
                    SerializedPageType::InstancePage(name)
                } else {
                    SerializedPageType::Instances
                }
            },
        }
    }

    fn from_serialized(serialized: &SerializedPageType, data: &DataEntities, cx: &App) -> Self {
        match serialized {
            SerializedPageType::Instances => PageType::Instances,
            SerializedPageType::Syncing => PageType::Syncing,
            SerializedPageType::Modrinth { installing_for } => {
                if let Some(installing_for) = installing_for {
                    if let Some(id) = InstanceEntries::find_id_by_name(&data.instances, installing_for, cx) {
                        return PageType::Modrinth { installing_for: Some(id) };
                    }
                }
                PageType::Modrinth { installing_for: None}
            },
            SerializedPageType::InstancePage(name) => {
                if let Some(id) = InstanceEntries::find_id_by_name(&data.instances, name, cx) {
                    PageType::InstancePage(id, InstanceSubpageType::Quickplay)
                } else {
                    PageType::Instances
                }
            },
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SerializedPageType {
    #[default]
    Instances,
    Syncing,
    Modrinth {
        installing_for: Option<SharedString>,
    },
    InstancePage(SharedString),
}

#[derive(Clone)]
pub enum LauncherPage {
    Instances(Entity<InstancesPage>),
    Syncing(Entity<SyncingPage>),
    Modrinth {
        installing_for: Option<InstanceID>,
        page: Entity<ModrinthSearchPage>,
    },
    InstancePage(InstanceID, InstanceSubpageType, Entity<InstancePage>),
}

impl LauncherPage {
    pub fn into_any_element(self) -> AnyElement {
        match self {
            LauncherPage::Instances(entity) => entity.into_any_element(),
            LauncherPage::Syncing(entity) => entity.into_any_element(),
            LauncherPage::Modrinth { page, .. } => page.into_any_element(),
            LauncherPage::InstancePage(_, _, entity) => entity.into_any_element(),
        }
    }

    pub fn page_type(&self) -> PageType {
        match self {
            LauncherPage::Instances(_) => PageType::Instances,
            LauncherPage::Syncing(_) => PageType::Syncing,
            LauncherPage::Modrinth { installing_for, .. } => PageType::Modrinth { installing_for: *installing_for },
            LauncherPage::InstancePage(id, subpage, _) => PageType::InstancePage(*id, *subpage),
        }
    }
}

impl LauncherUI {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar_state = cx.new(|_| ResizableState::default());

        let recent_instances = data
            .instances
            .read(cx)
            .entries
            .iter()
            .take(3)
            .map(|(id, ent)| (*id, ent.read(cx).name.clone()))
            .collect();

        let _instance_added_subscription =
            cx.subscribe::<_, InstanceAddedEvent>(&data.instances, |this, _, event, cx| {
                if this.recent_instances.is_full() {
                    this.recent_instances.pop();
                }
                let _ = this.recent_instances.insert(0, (event.instance.id, event.instance.name.clone()));
                cx.notify();
            });
        let _instance_modified_subscription =
            cx.subscribe::<_, InstanceModifiedEvent>(&data.instances, |this, _, event, cx| {
                if let Some((_, name)) = this.recent_instances.iter_mut().find(|(id, _)| *id == event.instance.id) {
                    *name = event.instance.name.clone();
                    cx.notify();
                }
                cx.notify();
            });
        let _instance_removed_subscription =
            cx.subscribe_in::<_, InstanceRemovedEvent>(&data.instances, window, |this, _, event, window, cx| {
                this.recent_instances.retain(|entry| entry.0 != event.id);
                if let LauncherPage::InstancePage(id, _, _) = this.page
                    && id == event.id
                {
                    this.switch_page(PageType::Instances, None, window, cx);
                }
                cx.notify();
            });
        let _instance_moved_to_top_subscription =
            cx.subscribe::<_, InstanceMovedToTopEvent>(&data.instances, |this, _, event, cx| {
                this.recent_instances.retain(|entry| entry.0 != event.instance.id);
                if this.recent_instances.is_full() {
                    this.recent_instances.pop();
                }
                let _ = this.recent_instances.insert(0, (event.instance.id, event.instance.name.clone()));
                cx.notify();
            });

        let page_type = PageType::from_serialized(&InterfaceConfig::get(cx).main_page, data, cx);

        Self {
            data: data.clone(),
            page: Self::create_page(&data, page_type, None, window, cx),
            sidebar_state,
            recent_instances,
            _instance_added_subscription,
            _instance_modified_subscription,
            _instance_removed_subscription,
            _instance_moved_to_top_subscription,
        }
    }

    fn create_page(data: &DataEntities, page: PageType, breadcrumb: Option<Box<dyn Fn() -> Breadcrumb>>, window: &mut Window, cx: &mut Context<Self>) -> LauncherPage {
        match page {
            PageType::Instances => {
                LauncherPage::Instances(cx.new(|cx| InstancesPage::new(data, window, cx)))
            },
            PageType::Syncing => {
                LauncherPage::Syncing(cx.new(|cx| SyncingPage::new(data, window, cx)))
            },
            PageType::Modrinth { installing_for } => {
                let breadcrumb = breadcrumb.unwrap_or(Box::new(|| Breadcrumb::new().text_xl()));
                let page = cx.new(|cx| {
                    ModrinthSearchPage::new(data, installing_for, breadcrumb, window, cx)
                });
                LauncherPage::Modrinth {
                    installing_for,
                    page,
                }
            },
            PageType::InstancePage(id, subpage) => {
                let breadcrumb = breadcrumb.unwrap_or(Box::new(|| Breadcrumb::new().text_xl().child(BreadcrumbItem::new("Instances").on_click(|_, window, cx| {
                    root::switch_page(PageType::Instances, None, window, cx);
                }))));
                LauncherPage::InstancePage(id, subpage, cx.new(|cx| {
                    InstancePage::new(id, subpage, data, breadcrumb, window, cx)
                }))
            },
        }
    }

    pub fn switch_page(&mut self, page: PageType, breadcrumb: Option<Box<dyn Fn() -> Breadcrumb>>, window: &mut Window, cx: &mut Context<Self>) {
        if self.page.page_type() == page {
            return;
        }
        InterfaceConfig::get_mut(cx).main_page = page.to_serialized(&self.data, cx);
        self.page = Self::create_page(&self.data, page, breadcrumb, window, cx);
        cx.notify();
    }
}

impl Render for LauncherUI {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page_type = self.page.page_type();

        let library_group = SidebarGroup::new("Library").child(
            SidebarMenu::new().children([
                SidebarMenuItem::new("Instances")
                    .active(page_type == PageType::Instances)
                    .on_click(cx.listener(|launcher, _, window, cx| {
                        launcher.switch_page(PageType::Instances, None, window, cx);
                    })),
                SidebarMenuItem::new("Syncing")
                    .active(page_type == PageType::Syncing)
                    .on_click(cx.listener(|launcher, _, window, cx| {
                        launcher.switch_page(PageType::Syncing, None, window, cx);
                    })),
            ]),
        );

        let launcher_group = SidebarGroup::new("Content").child(
            SidebarMenu::new().children([SidebarMenuItem::new("Modrinth")
                .active(page_type == PageType::Modrinth { installing_for: None })
                .on_click(cx.listener(|launcher, _, window, cx| {
                    launcher.switch_page(PageType::Modrinth { installing_for: None }, None, window, cx);
                }))]),
        );

        let mut groups: heapless::Vec<SidebarGroup<SidebarMenu>, 3> = heapless::Vec::new();

        let _ = groups.push(library_group);
        let _ = groups.push(launcher_group);

        if !self.recent_instances.is_empty() {
            let recent_instances_group = SidebarGroup::new("Recent Instances").child(SidebarMenu::new().children(
                self.recent_instances.iter().map(|(id, name)| {
                    let name = name.clone();
                    let id = *id;
                    let active = if let PageType::InstancePage(page_id, _) = page_type {
                        page_id == id
                    } else {
                        false
                    };
                    SidebarMenuItem::new(name)
                        .active(active)
                        .on_click(cx.listener(move |launcher, _, window, cx| {
                            launcher.switch_page(PageType::InstancePage(id, InstanceSubpageType::Quickplay), None, window, cx);
                        }))
                }),
            ));
            let _ = groups.push(recent_instances_group);
        }

        let accounts = self.data.accounts.read(cx);
        let (account_head, account_name) = if let Some(account) = &accounts.selected_account {
            let account_name = SharedString::new(account.username.clone());
            let head = if let Some(head) = &account.head {
                let resize = png_render_cache::ImageTransformation::Resize { width: 32, height: 32 };
                png_render_cache::render_with_transform(Arc::clone(head), resize, cx)
            } else {
                gpui::img(ImageSource::Resource(Resource::Embedded("images/default_head.png".into())))
            };
            (head, account_name)
        } else {
            (
                gpui::img(ImageSource::Resource(Resource::Embedded("images/default_head.png".into()))),
                "No Account".into(),
            )
        };

        let pandora_icon = Icon::empty().path("icons/pandora.svg");

        let footer = div().flex_grow().id("footer-button").child(SidebarFooter::new()
            .w_full()
            .justify_center()
            .text_size(rems(0.9375))
            .child(account_head.size_8().min_w_8().min_h_8())
            .child(account_name))
            .on_click({
                let accounts = self.data.accounts.clone();
                let backend_handle = self.data.backend_handle.clone();
                move |_, window, cx| {
                    if accounts.read(cx).accounts.is_empty() {
                        crate::root::start_new_account_login(&backend_handle, window, cx);
                        return;
                    }

                    let accounts = accounts.clone();
                    let backend_handle = backend_handle.clone();
                    window.open_sheet_at(gpui_component::Placement::Left, cx, move |sheet, window, cx| {
                        let (accounts, selected_account) = {
                            let accounts = accounts.read(cx);
                            (accounts.accounts.clone(), accounts.selected_account_uuid)
                        };

                        let items = accounts.iter().map(|account| {
                            let head = if let Some(head) = &account.head {
                                let resize = png_render_cache::ImageTransformation::Resize { width: 32, height: 32 };
                                png_render_cache::render_with_transform(Arc::clone(head), resize, cx)
                            } else {
                                gpui::img(ImageSource::Resource(Resource::Embedded("images/default_head.png".into())))
                            };
                            let account_name = SharedString::new(account.username.clone());

                            let selected = Some(account.uuid) == selected_account;

                            Button::new(account_name.clone())
                                .when(selected, |this| {
                                    this.info()
                                })
                                .h_10()
                                .child(head.size_8().min_w_8().min_h_8())
                                .child(account_name)
                                .when(!selected, |this| {
                                    this.on_click({
                                        let backend_handle = backend_handle.clone();
                                        let uuid = account.uuid;
                                        move |_, _, _| {
                                            backend_handle.send(MessageToBackend::SelectAccount { uuid });
                                        }
                                    })
                                })
                        });

                        sheet
                            .title("Accounts")
                            .overlay_top(crate::root::sheet_margin_top(window))
                            .gap_2()
                            .child(Button::new("Add account").h_10().success().icon(IconName::Plus).label("Add account").on_click({
                                let backend_handle = backend_handle.clone();
                                move |_, window, cx| {
                                    crate::root::start_new_account_login(&backend_handle, window, cx);
                                }
                            }))
                            .children(items)
                    });
                }
            });

        let settings_button = div()
            .id("settings-button")
            .gap_2()
            .p_2()
            .rounded(cx.theme().radius)
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .child(IconName::Settings)
            .on_click({
                let data = self.data.clone();
                move |_, window, cx| {
                    let build = modals::settings::build_settings_sheet(&data, window, cx);
                    window.open_sheet_at(gpui_component::Placement::Left, cx, build);
                }
            });

        let sidebar = Sidebar::left()
            .w(relative(1.))
            .border_0()
            .header(
                h_flex()
                    .p_2()
                    .gap_2()
                    .w_full()
                    .justify_center()
                    .text_size(rems(0.9375))
                    .child(pandora_icon.size_8().min_w_8().min_h_8())
                    .child("Pandora"),
            )
            .footer(h_flex().flex_wrap().justify_center().w_full().child(settings_button).child(footer))
            .children(groups);

        h_resizable("container")
            .with_state(&self.sidebar_state)
            .child(resizable_panel().size(px(150.)).size_range(px(130.)..px(200.)).child(sidebar))
            .child(self.page.clone().into_any_element())
    }
}

pub fn page(cx: &App, title: impl IntoElement) -> gpui::Div {
    v_flex().size_full().child(
        h_flex()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xl()
            .child(div().left_4().child(title)),
    )
}
