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
    entity::{DataEntities, instance::InstanceEntry}, icon::PandoraIcon, interface_config::InterfaceConfig, pages::{instance::{content_subpage::InstanceContentSubpage, logs_subpage::InstanceLogsSubpage, quickplay_subpage::InstanceQuickplaySubpage, settings_subpage::InstanceSettingsSubpage, terminal_subpage::InstanceTerminalSubpage}, page::Page}, root,
};

use super::content_subpage::ContentType;

pub struct InstancePage {
    backend_handle: BackendHandle,
    data: DataEntities,
    pub instance: Entity<InstanceEntry>,
    subpage: InstanceSubpage,
}

impl InstancePage {
    pub fn new(instance_id: InstanceID, data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance = data.instances.read(cx).entries.get(&instance_id).unwrap().clone();

        cx.observe(&instance, |_, _, cx| cx.notify()).detach();

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
                Button::new("start_instance").success().icon(PandoraIcon::Play).label(t::instance::start::label()).on_click(
                    move |_, window, cx| {
                        root::start_instance(id, name.clone(), None, &backend_handle, window, cx);
                    },
                ).into_any_element()
            },
            InstanceStatus::Launching => {
                Button::new("launching").warning().icon(PandoraIcon::Loader).label(t::instance::start::starting()).into_any_element()
            },
            InstanceStatus::Stopping => {
                Button::new("stopping")
                    .danger()
                    .icon(PandoraIcon::Loader)
                    .label(t::instance::start::stopping())
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
                        .label(t::instance::kill_instance())
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
                                dialog.title(t::instance::already_running::title())
                                    .overlay_closable(false)
                                    .flex()
                                    .line_height(rems(1.2))
                                    .child(t::instance::already_running::body())
                                    .child(div().h_2())
                                    .child(t::instance::already_running::body2())
                                    .footer(h_flex()
                                        .gap_2()
                                        .w_full()
                                        .child(
                                            Button::new("cancel")
                                                .label(t::common::cancel())
                                                .on_click(|_, window, cx| {
                                                    window.close_dialog(cx);
                                                }).flex_grow()
                                        )
                                        .child(
                                            Button::new("ok")
                                                .success()
                                                .label(t::instance::already_running::start_anyway())
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
            .label(t::instance::open_folder())
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
        let mut instance_subpage = InterfaceConfig::get(cx).instance_subpage;

        let global_terminal_in_tab = InterfaceConfig::get(cx).terminal_in_tab;
        let effective_terminal_in_tab = self.instance.read(cx).configuration.terminal_in_tab.unwrap_or(global_terminal_in_tab);

        // The Terminal tab is shown whenever the instance's effective mode is "tab" (live, even
        // before the first launch). If a "Terminal" selection is active but the effective mode is no
        // longer tab, fall back to Settings.
        if instance_subpage == InstanceSubpageType::Terminal && !effective_terminal_in_tab {
            instance_subpage = InstanceSubpageType::Settings;
        }

        if instance_subpage != self.subpage.page_type() {
            self.subpage = instance_subpage.create(&self.instance, &self.data, self.backend_handle.clone(), window, cx);
        }

        let show_shader_tab = self.instance.read(cx).configuration.show_shader_tab || matches!(self.subpage, InstanceSubpage::Shaders(_));
        let show_terminal_tab = effective_terminal_in_tab;

        let mut tabs: Vec<(InstanceSubpageType, SharedString)> = vec![
            (InstanceSubpageType::Quickplay, t::instance::quickplay().into()),
            (InstanceSubpageType::Logs, t::instance::logs::title().into()),
            (InstanceSubpageType::Mods, t::instance::content::mods().into()),
            (InstanceSubpageType::ResourcePacks, t::instance::content::resourcepacks().into()),
        ];
        if show_shader_tab {
            tabs.push((InstanceSubpageType::Shaders, t::instance::content::shaders().into()));
        }
        tabs.push((InstanceSubpageType::Settings, t::settings::title().into()));
        if show_terminal_tab {
            tabs.push((InstanceSubpageType::Terminal, t::instance::terminal::title().into()));
        }

        let current_type = self.subpage.page_type();
        let selected_index = tabs.iter().position(|(ty, _)| *ty == current_type).unwrap_or(0);
        let tab_types: Vec<InstanceSubpageType> = tabs.iter().map(|(ty, _)| *ty).collect();

        let mut tab_bar = TabBar::new("bar")
            .prefix(div().w_4())
            .selected_index(selected_index)
            .underline();
        for (_, label) in &tabs {
            tab_bar = tab_bar.child(Tab::new().label(label.clone()));
        }
        let tab_bar = tab_bar.on_click(cx.listener(move |_, index: &usize, _, cx| {
            if let Some(page_type) = tab_types.get(*index) {
                InterfaceConfig::get_mut(cx).instance_subpage = *page_type;
            }
        }));

        v_flex()
            .size_full()
            .child(tab_bar)
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
    Shaders,
    Settings,
    Terminal,
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
                InstanceContentSubpage::new(instance, ContentType::Mods, backend_handle, window, cx)
            })),
            InstanceSubpageType::ResourcePacks => InstanceSubpage::ResourcePacks(cx.new(|cx| {
                InstanceContentSubpage::new(instance, ContentType::ResourcePacks, backend_handle, window, cx)
            })),
            InstanceSubpageType::Shaders => InstanceSubpage::Shaders(cx.new(|cx| {
                InstanceContentSubpage::new(instance, ContentType::Shaders, backend_handle, window, cx)
            })),
            InstanceSubpageType::Settings => InstanceSubpage::Settings(cx.new(|cx| {
                InstanceSettingsSubpage::new(instance, data, backend_handle, window, cx)
            })),
            InstanceSubpageType::Terminal => InstanceSubpage::Terminal(cx.new(|cx| {
                InstanceTerminalSubpage::new(instance, window, cx)
            })),
        }
    }
}

#[derive(Clone)]
pub enum InstanceSubpage {
    Quickplay(Entity<InstanceQuickplaySubpage>),
    Logs(Entity<InstanceLogsSubpage>),
    Mods(Entity<InstanceContentSubpage>),
    ResourcePacks(Entity<InstanceContentSubpage>),
    Shaders(Entity<InstanceContentSubpage>),
    Settings(Entity<InstanceSettingsSubpage>),
    Terminal(Entity<InstanceTerminalSubpage>),
}

impl InstanceSubpage {
    pub fn page_type(&self) -> InstanceSubpageType {
        match self {
            InstanceSubpage::Quickplay(_) => InstanceSubpageType::Quickplay,
            InstanceSubpage::Logs(_) => InstanceSubpageType::Logs,
            InstanceSubpage::Mods(_) => InstanceSubpageType::Mods,
            InstanceSubpage::ResourcePacks(_) => InstanceSubpageType::ResourcePacks,
            InstanceSubpage::Shaders(_) => InstanceSubpageType::Shaders,
            InstanceSubpage::Settings(_) => InstanceSubpageType::Settings,
            InstanceSubpage::Terminal(_) => InstanceSubpageType::Terminal,
        }
    }

    pub fn into_any_element(self) -> AnyElement {
        match self {
            Self::Quickplay(entity) => entity.into_any_element(),
            Self::Logs(entity) => entity.into_any_element(),
            Self::Mods(entity) => entity.into_any_element(),
            Self::ResourcePacks(entity) => entity.into_any_element(),
            Self::Shaders(entity) => entity.into_any_element(),
            Self::Settings(entity) => entity.into_any_element(),
            Self::Terminal(entity) => entity.into_any_element(),
        }
    }
}
