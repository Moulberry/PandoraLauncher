use bridge::{
    handle::BackendHandle,
    instance::{InstanceID, InstanceStatus},
    message::MessageToBackend,
};
use gpui::{prelude::*, *};
use gpui_component::{
    WindowExt, button::{Button, ButtonGroup, ButtonVariants}, h_flex, tab::{Tab, TabBar}, v_flex
};
use serde::{Deserialize, Serialize};

use crate::{
    entity::{DataEntities, instance::InstanceEntry}, icon::PandoraIcon, interface_config::InterfaceConfig, pages::{instance::{logs_subpage::InstanceLogsSubpage, mods_subpage::InstanceModsSubpage, quickplay_subpage::InstanceQuickplaySubpage, resource_packs_subpage::InstanceResourcePacksSubpage, settings_subpage::InstanceSettingsSubpage}, page::Page}, root, ts
};

pub struct InstancePage {
    backend_handle: BackendHandle,
    data: DataEntities,
    pub instance: Entity<InstanceEntry>,
    subpage: InstanceSubpage,
}

impl InstancePage {
    pub fn new(instance_id: InstanceID, data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance = data.instances.read(cx).entries.get(&instance_id).unwrap().clone();

        let instance_subpage = InterfaceConfig::get(cx).instance_subpage;
        let subpage = instance_subpage.create(&instance, data, data.backend_handle.clone(), window, cx);

        Self {
            backend_handle: data.backend_handle.clone(),
            data: data.clone(),
            instance,
            subpage,
        }
    }
}

impl Page for InstancePage {
    fn controls(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let instance = self.instance.read(cx);
        let id = instance.id;
        let name = instance.name.clone();
        let backend_handle = self.backend_handle.clone();

        let button = match instance.status {
            InstanceStatus::NotRunning => {
                Button::new("start_instance").success().icon(PandoraIcon::Play).label(ts!("instance.start.label")).on_click(
                    move |_, window, cx| {
                        root::start_instance(id, name.clone(), None, &backend_handle, window, cx);
                    },
                ).into_any_element()
            },
            InstanceStatus::Launching => {
                Button::new("launching").warning().icon(PandoraIcon::Loader).label(ts!("instance.start.starting")).into_any_element()
            },
            InstanceStatus::Stopping => {
                Button::new("stopping")
                    .danger()
                    .icon(PandoraIcon::Loader)
                    .label(ts!("instance.start.stopping"))
                    .on_click({
                        let backend_handle = backend_handle.clone();
                        move |_, _, _| {
                            backend_handle.send(MessageToBackend::KillInstance { id });
                        }
                    })
                    .into_any_element()
            },
            InstanceStatus::Running => {
                ButtonGroup::new("running")
                    .child(Button::new("kill_instance")
                        .danger()
                        .icon(PandoraIcon::Close)
                        .label(ts!("instance.kill_instance"))
                        .on_click({
                            let backend_handle = backend_handle.clone();
                            move |_, _, _| {
                                backend_handle.send(MessageToBackend::KillInstance { id });
                            }
                        }))
                    .child(Button::new("start_again")
                        .success()
                        .icon(PandoraIcon::Play)
                        .on_click(move |_, window, cx| {
                            let name = name.clone();
                            let backend_handle = backend_handle.clone();
                            window.open_dialog(cx, move |dialog, _, _| {
                                dialog.title("Instance already running")
                                    .overlay_closable(false)
                                    .flex()
                                    .line_height(rems(1.2))
                                    .child("Starting it again may cause malfunction or corrupt your saved worlds.")
                                    .child(div().h_2())
                                    .child("We cannot take responsibility for any issues if you choose to start another game. Would you like to continue anyway?")
                                    .footer(h_flex()
                                        .gap_2()
                                        .w_full()
                                        .child(
                                            Button::new("cancel")
                                                .label("Cancel")
                                                .on_click(|_, window, cx| {
                                                    window.close_dialog(cx);
                                                }).flex_grow()
                                        )
                                        .child(
                                            Button::new("ok")
                                                .success()
                                                .label("Start anyway")
                                                .on_click({
                                                    let name = name.clone();
                                                    let backend_handle = backend_handle.clone();
                                                    move |_, window, cx| {
                                                        window.close_dialog(cx);
                                                        root::start_instance(id, name.clone(), None, &backend_handle, window, cx);
                                                    }
                                                })
                                        ))
                            })
                        })).into_any_element()
            },
        };

        let open_dot_minecraft_button = Button::new("open_dot_minecraft")
            .info()
            .icon(PandoraIcon::FolderOpen)
            .label(ts!("instance.open_folder"))
            .on_click({
            let dot_minecraft = instance.dot_minecraft_folder.clone();
            move |_, window, cx| {
                crate::open_folder(&dot_minecraft, window, cx);
            }
        });

        h_flex().gap_3().child(button).child(open_dot_minecraft_button)
    }

    fn scrollable(&self, _cx: &App) -> bool {
        false
    }
}

impl Render for InstancePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let instance_subpage = InterfaceConfig::get(cx).instance_subpage;
        if instance_subpage != self.subpage.page_type() {
            self.subpage = instance_subpage.create(&self.instance, &self.data, self.backend_handle.clone(), window, cx);
        }

        let selected_index = match &self.subpage {
            InstanceSubpage::Quickplay(_) => 0,
            InstanceSubpage::Logs(_) => 1,
            InstanceSubpage::Mods(_) => 2,
            InstanceSubpage::ResourcePacks(_) => 3,
            InstanceSubpage::Settings(_) => 4,
        };

        v_flex()
            .size_full()
            .child(
                TabBar::new("bar")
                    .prefix(div().w_4())
                    .selected_index(selected_index)
                    .underline()
                    .child(Tab::new().label(ts!("instance.quickplay")))
                    .child(Tab::new().label(ts!("instance.logs.title")))
                    .child(Tab::new().label(ts!("instance.content.mods")))
                    .child(Tab::new().label(ts!("instance.content.resourcepacks")))
                    .child(Tab::new().label(ts!("settings.title")))
                    .on_click(cx.listener(|_, index, _, cx| {
                        let page_type = match *index {
                            0 => InstanceSubpageType::Quickplay,
                            1 => InstanceSubpageType::Logs,
                            2 => InstanceSubpageType::Mods,
                            3 => InstanceSubpageType::ResourcePacks,
                            4 => InstanceSubpageType::Settings,
                            _ => {
                                return;
                            },
                        };
                        InterfaceConfig::get_mut(cx).instance_subpage = page_type;
                    })),
            )
            .child(self.subpage.clone().into_any_element())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceSubpageType {
    #[default]
    Quickplay,
    Logs,
    Mods,
    ResourcePacks,
    Settings,
}

impl InstanceSubpageType {
    pub fn create(
        self,
        instance: &Entity<InstanceEntry>,
        data: &DataEntities,
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
            InstanceSubpageType::ResourcePacks => InstanceSubpage::ResourcePacks(cx.new(|cx| {
                InstanceResourcePacksSubpage::new(instance, backend_handle, window, cx)
            })),
            InstanceSubpageType::Settings => InstanceSubpage::Settings(cx.new(|cx| {
                InstanceSettingsSubpage::new(instance, data, backend_handle, window, cx)
            })),
        }
    }
}

#[derive(Clone)]
pub enum InstanceSubpage {
    Quickplay(Entity<InstanceQuickplaySubpage>),
    Logs(Entity<InstanceLogsSubpage>),
    Mods(Entity<InstanceModsSubpage>),
    ResourcePacks(Entity<InstanceResourcePacksSubpage>),
    Settings(Entity<InstanceSettingsSubpage>),
}

impl InstanceSubpage {
    pub fn page_type(&self) -> InstanceSubpageType {
        match self {
            InstanceSubpage::Quickplay(_) => InstanceSubpageType::Quickplay,
            InstanceSubpage::Logs(_) => InstanceSubpageType::Logs,
            InstanceSubpage::Mods(_) => InstanceSubpageType::Mods,
            InstanceSubpage::ResourcePacks(_) => InstanceSubpageType::ResourcePacks,
            InstanceSubpage::Settings(_) => InstanceSubpageType::Settings,
        }
    }

    pub fn into_any_element(self) -> AnyElement {
        match self {
            Self::Quickplay(entity) => entity.into_any_element(),
            Self::Logs(entity) => entity.into_any_element(),
            Self::Mods(entity) => entity.into_any_element(),
            Self::ResourcePacks(entity) => entity.into_any_element(),
            Self::Settings(entity) => entity.into_any_element(),
        }
    }
}
