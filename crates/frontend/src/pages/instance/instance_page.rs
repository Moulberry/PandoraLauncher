use bridge::{
    handle::BackendHandle,
    instance::{InstanceID, InstanceStatus},
    message::MessageToBackend,
};
use gpui::{prelude::*, *};
use gpui_component::{
    breadcrumb::Breadcrumb, button::{Button, ButtonVariants}, h_flex, tab::{Tab, TabBar}, Icon, IconName
};
use serde::{Deserialize, Serialize};

use crate::{
    entity::{instance::InstanceEntry, DataEntities},
    pages::instance::{logs_subpage::InstanceLogsSubpage, mods_subpage::InstanceModsSubpage, quickplay_subpage::InstanceQuickplaySubpage, settings_subpage::InstanceSettingsSubpage},
    root, ui,
};

pub struct InstancePage {
    breadcrumb: Box<dyn Fn() -> Breadcrumb>,
    backend_handle: BackendHandle,
    title: SharedString,
    instance: Entity<InstanceEntry>,
    subpage: InstanceSubpage,
    _instance_subscription: Subscription,
}

impl InstancePage {
    pub fn new(instance_id: InstanceID, subpage: InstanceSubpageType, data: &DataEntities, breadcrumb: Box<dyn Fn() -> Breadcrumb>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance = data.instances.read(cx).entries.get(&instance_id).unwrap().clone();

        let _instance_subscription = cx.observe(&instance, |page, instance, cx| {
            let instance = instance.read(cx);
            page.title = instance.title().into();
        });

        let subpage = subpage.create(&instance, data.backend_handle.clone(), window, cx);

        Self {
            breadcrumb,
            backend_handle: data.backend_handle.clone(),
            title: instance.read(cx).title().into(),
            instance,
            subpage,
            _instance_subscription,
        }
    }

    fn set_subpage(&mut self, page_type: InstanceSubpageType, window: &mut Window, cx: &mut Context<Self>) {
        if page_type == self.subpage.page_type() {
            return;
        }
        self.subpage = page_type.create(&self.instance, self.backend_handle.clone(), window, cx);
    }
}

impl Render for InstancePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_index = match &self.subpage {
            InstanceSubpage::Quickplay(_) => 0,
            InstanceSubpage::Logs(_) => 1,
            InstanceSubpage::Mods(_) => 2,
            InstanceSubpage::Settings(_) => 3,
        };

        let play_icon = Icon::empty().path("icons/play.svg");

        let instance = self.instance.read(cx);
        let id = instance.id;
        let name = instance.name.clone();
        let backend_handle = self.backend_handle.clone();

        let button = match instance.status {
            InstanceStatus::NotRunning => {
                Button::new("start_instance").success().icon(play_icon).label("Start Instance").on_click(
                    move |_, window, cx| {
                        root::start_instance(id, name.clone(), None, &backend_handle, window, cx);
                    },
                )
            },
            InstanceStatus::Launching => {
                Button::new("launching").warning().icon(IconName::Loader).label("Launching...")
            },
            InstanceStatus::Running => Button::new("kill_instance")
                .danger()
                .icon(IconName::Close)
                .label("Kill Instance")
                .on_click(move |_, _, _| {
                    backend_handle.send(MessageToBackend::KillInstance { id });
                }),
        };

        let open_game_output = Button::new("open_game_output")
            .info()
            .icon(IconName::BookOpen)
            .label("Open Game Output")
            .on_click({
                let backend_handle = self.backend_handle.clone();
                let instance_id = instance.id;
                move |_, window, cx| {
                    backend_handle.send(MessageToBackend::ShowGameOutputWindow {
                        instance: instance_id,
                    });
                }
            });


        let open_dot_minecraft_button = Button::new("open_dot_minecraft")
            .info()
            .icon(IconName::FolderOpen)
            .label("Open .minecraft folder")
            .on_click({
            let dot_minecraft = instance.dot_minecraft_folder.clone();
            move |_, window, cx| {
                crate::open_folder(&dot_minecraft, window, cx);
            }
        });

        let breadcrumb = (self.breadcrumb)().child(self.title.clone());
        ui::page(cx, h_flex().gap_8().child(breadcrumb).child(h_flex().gap_3().child(button).child(open_dot_minecraft_button).child(h_flex().gap_3().child(open_game_output))))
            .child(
                TabBar::new("bar")
                    .prefix(div().w_4())
                    .selected_index(selected_index)
                    .underline()
                    .child(Tab::new().label("Quickplay"))
                    .child(Tab::new().label("Logs"))
                    .child(Tab::new().label("Mods"))
                    .child(Tab::new().label("Settings"))
                    .on_click(cx.listener(|page, index, window, cx| {
                        let page_type = match *index {
                            0 => InstanceSubpageType::Quickplay,
                            1 => InstanceSubpageType::Logs,
                            2 => InstanceSubpageType::Mods,
                            3 => InstanceSubpageType::Settings,
                            _ => {
                                return;
                            },
                        };
                        page.set_subpage(page_type, window, cx);
                    })),
            )
            .child(self.subpage.clone().into_any_element())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceSubpageType {
    Quickplay,
    Logs,
    Mods,
    Settings,
}

impl InstanceSubpageType {
    pub fn create(
        self,
        instance: &Entity<InstanceEntry>,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut App
    ) -> InstanceSubpage {
        match self {
            InstanceSubpageType::Quickplay => InstanceSubpage::Quickplay(cx.new(|cx| {
                InstanceQuickplaySubpage::new(instance, backend_handle, window, cx)
            })),
            InstanceSubpageType::Logs => InstanceSubpage::Logs(cx.new(|cx| {
                InstanceLogsSubpage::new(instance, backend_handle, window, cx)
            })),
            InstanceSubpageType::Mods => InstanceSubpage::Mods(cx.new(|cx| {
                InstanceModsSubpage::new(instance, backend_handle, window, cx)
            })),
            InstanceSubpageType::Settings => InstanceSubpage::Settings(cx.new(|cx| {
                InstanceSettingsSubpage::new(instance, backend_handle, window, cx)
            })),
        }
    }
}

#[derive(Clone)]
pub enum InstanceSubpage {
    Quickplay(Entity<InstanceQuickplaySubpage>),
    Logs(Entity<InstanceLogsSubpage>),
    Mods(Entity<InstanceModsSubpage>),
    Settings(Entity<InstanceSettingsSubpage>),
}

impl InstanceSubpage {
    pub fn page_type(&self) -> InstanceSubpageType {
        match self {
            InstanceSubpage::Quickplay(_) => InstanceSubpageType::Quickplay,
            InstanceSubpage::Logs(_) => InstanceSubpageType::Logs,
            InstanceSubpage::Mods(_) => InstanceSubpageType::Mods,
            InstanceSubpage::Settings(_) => InstanceSubpageType::Settings,
        }
    }

    pub fn into_any_element(self) -> AnyElement {
        match self {
            Self::Quickplay(entity) => entity.into_any_element(),
            Self::Logs(entity) => entity.into_any_element(),
            Self::Mods(entity) => entity.into_any_element(),
            Self::Settings(entity) => entity.into_any_element(),
        }
    }
}
