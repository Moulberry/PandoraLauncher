use std::{ffi::OsString, sync::Arc};

use bridge::{
    handle::BackendHandle,
    instance::{InstanceID, InstanceServerSummary, InstanceWorldSummary},
    message::{BridgeDataLoadState, MessageToBackend, QuickPlayLaunch},
    serial::AtomicOptionSerial,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, IndexPath,
    button::{Button, ButtonVariants},
    h_flex,
    list::{ListDelegate, ListItem, ListState},
    v_flex,
};

use crate::{
    entity::instance::InstanceEntry, icon::PandoraIcon, interface_config::InterfaceConfig, png_render_cache, root, ts,
};

pub struct InstanceQuickplaySubpage {
    instance: Entity<InstanceEntry>,
    backend_handle: BackendHandle,
    worlds_state: BridgeDataLoadState,
    world_list: Entity<ListState<WorldsListDelegate>>,
    servers_state: BridgeDataLoadState,
    server_list: Entity<ListState<ServersListDelegate>>,
    worlds_serial: AtomicOptionSerial,
    servers_serial: AtomicOptionSerial,
}

impl InstanceQuickplaySubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        backend_handle: BackendHandle,
        mut window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let instance_entity = instance.clone();
        let instance = instance.read(cx);
        let instance_id = instance.id;

        let worlds_state = instance.worlds_state.clone();
        let servers_state = instance.servers_state.clone();

        let worlds_list_delegate = WorldsListDelegate {
            id: instance_id,
            name: instance.name.clone(),
            backend_handle: backend_handle.clone(),
            worlds: instance.worlds.read(cx).to_vec(),
            searched: instance.worlds.read(cx).to_vec(),
        };

        let servers_list_delegate = ServersListDelegate {
            id: instance_id,
            name: instance.name.clone(),
            backend_handle: backend_handle.clone(),
            servers: instance.servers.read(cx).to_vec(),
            searched: instance.servers.read(cx).to_vec(),
        };

        let worlds = instance.worlds.clone();
        let servers = instance.servers.clone();

        let window2 = &mut window;
        let world_list = cx.new(move |cx| {
            cx.observe(&worlds, |list: &mut ListState<WorldsListDelegate>, worlds, cx| {
                let worlds = worlds.read(cx).to_vec();
                let delegate = list.delegate_mut();
                delegate.worlds = worlds.clone();
                delegate.searched = worlds;
                cx.notify();
            })
            .detach();

            ListState::new(worlds_list_delegate, window2, cx).selectable(false).searchable(true)
        });

        let server_list = cx.new(move |cx| {
            cx.observe(&servers, |list: &mut ListState<ServersListDelegate>, servers, cx| {
                let servers = servers.read(cx).to_vec();
                let delegate = list.delegate_mut();
                delegate.servers = servers.clone();
                delegate.searched = servers;
                cx.notify();
            })
            .detach();

            ListState::new(servers_list_delegate, window, cx).selectable(false).searchable(true)
        });

        cx.observe(&instance_entity, |_, _, cx| {
            cx.notify();
        })
        .detach();

        Self {
            instance: instance_entity,
            backend_handle,
            worlds_state,
            world_list,
            servers_state,
            server_list,
            worlds_serial: AtomicOptionSerial::default(),
            servers_serial: AtomicOptionSerial::default(),
        }
    }
}

impl Render for InstanceQuickplaySubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();
        let instance = self.instance.read(cx);
        let playtime = instance.playtime;
        let instance_id = instance.id;

        self.worlds_state.set_observed();
        if self.worlds_state.should_load() {
            self.backend_handle
                .send_with_serial(MessageToBackend::RequestLoadWorlds { id: instance_id }, &self.worlds_serial);
        }

        self.servers_state.set_observed();
        if self.servers_state.should_load() {
            self.backend_handle
                .send_with_serial(MessageToBackend::RequestLoadServers { id: instance_id }, &self.servers_serial);
        }

        let worlds_header = div().mb_1().ml_1().text_lg().child(ts!("instance.worlds"));
        let servers_header = div().mb_1().ml_1().text_lg().child(ts!("instance.servers"));
        let total_playtime = format_playtime(playtime.total_secs);
        let current_session = if playtime.current_session_secs > 0 {
            format_playtime(playtime.current_session_secs)
        } else {
            "Not running".into()
        };

        v_flex()
            .p_4()
            .gap_4()
            .size_full()
            .child(
                h_flex()
                    .gap_4()
                    .child(card(
                        ts!("instance.current_session"),
                        current_session.into(),
                        theme.border,
                        theme.radius,
                    ))
                    .child(card(
                        ts!("instance.total_playtime"),
                        total_playtime,
                        theme.border,
                        theme.radius,
                    )),
            )
            .child(
                h_flex()
                    .size_full()
                    .gap_4()
                    .child(
                        v_flex().size_full().child(worlds_header).child(
                            v_flex()
                                .text_base()
                                .size_full()
                                .border_1()
                                .rounded(theme.radius)
                                .border_color(theme.border)
                                .child(self.world_list.clone()),
                        ),
                    )
                    .child(
                        v_flex().size_full().child(servers_header).child(
                            v_flex()
                                .text_base()
                                .size_full()
                                .border_1()
                                .rounded(theme.radius)
                                .border_color(theme.border)
                                .child(self.server_list.clone()),
                        ),
                    ),
            )
    }
}

fn card(label: impl Into<SharedString>, value: SharedString, border: Hsla, radius: Pixels) -> Div {
    v_flex()
        .gap_1()
        .px_3()
        .py_2()
        .min_w_40()
        .border_1()
        .border_color(border)
        .rounded(radius)
        .child(div().text_sm().opacity(0.6).child(label.into()))
        .child(div().text_lg().child(value))
}

fn format_playtime(total_secs: u64) -> SharedString {
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m").into()
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s").into()
    } else {
        format!("{seconds}s").into()
    }
}

pub struct WorldsListDelegate {
    id: InstanceID,
    name: SharedString,
    backend_handle: BackendHandle,
    worlds: Vec<InstanceWorldSummary>,
    searched: Vec<InstanceWorldSummary>,
}

impl ListDelegate for WorldsListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.searched.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let summary = self.searched.get(ix.row)?;

        let icon = if let Some(png_icon) = summary.png_icon.as_ref() {
            png_render_cache::render(Arc::clone(png_icon), cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded("images/default_world.png".into())))
        };

        let description = v_flex().child(SharedString::from(summary.title.clone())).child(
            div()
                .text_color(Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.5,
                    a: 1.0,
                })
                .child(SharedString::from(summary.subtitle.clone())),
        );

        let id = self.id;
        let name = self.name.clone();
        let backend_handle = self.backend_handle.clone();
        let target = summary.level_path.file_name().unwrap().to_owned();
        let item = ListItem::new(ix).p_1().child(
            h_flex()
                .gap_1()
                .child(
                    div()
                        .child(Button::new(ix).success().icon(PandoraIcon::Play).on_click(move |_, window, cx| {
                            root::start_instance(
                                id,
                                name.clone(),
                                Some(QuickPlayLaunch::Singleplayer(target.clone())),
                                &backend_handle,
                                window,
                                cx,
                            );
                        }))
                        .px_2(),
                )
                .child(icon.size_16().min_w_16().min_h_16())
                .child(description),
        );

        Some(item)
    }

    fn set_selected_index(&mut self, _ix: Option<IndexPath>, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {
    }

    fn perform_search(&mut self, query: &str, _window: &mut Window, _cx: &mut Context<ListState<Self>>) -> Task<()> {
        self.searched = self.worlds.iter().filter(|w| w.title.contains(query)).cloned().collect();

        Task::ready(())
    }
}

pub struct ServersListDelegate {
    id: InstanceID,
    name: SharedString,
    backend_handle: BackendHandle,
    servers: Vec<InstanceServerSummary>,
    searched: Vec<InstanceServerSummary>,
}

impl ListDelegate for ServersListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.searched.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let interface_config = InterfaceConfig::get(cx);

        let interface_config_hide_server_addresses = interface_config.hide_server_addresses;

        let summary = self.searched.get(ix.row)?;

        let icon = if let Some(png_icon) = summary.png_icon.as_ref() {
            png_render_cache::render(Arc::clone(png_icon), cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded("images/default_world.png".into())))
        };

        let description = v_flex().child(SharedString::from(summary.name.clone())).when(
            !interface_config_hide_server_addresses,
            |parent| {
                parent.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(summary.ip.clone())),
                )
            },
        );

        let id = self.id;
        let name = self.name.clone();
        let backend_handle = self.backend_handle.clone();
        let target = OsString::from(summary.ip.to_string());
        let item = ListItem::new(ix).p_1().child(
            h_flex()
                .gap_1()
                .child(
                    div()
                        .child(Button::new(ix).success().icon(PandoraIcon::Play).on_click(move |_, window, cx| {
                            root::start_instance(
                                id,
                                name.clone(),
                                Some(QuickPlayLaunch::Multiplayer(target.clone())),
                                &backend_handle,
                                window,
                                cx,
                            );
                        }))
                        .px_2(),
                )
                .child(icon.size_16().min_w_16().min_h_16())
                .child(description),
        );

        Some(item)
    }

    fn set_selected_index(&mut self, _ix: Option<IndexPath>, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {
    }

    fn perform_search(&mut self, query: &str, _window: &mut Window, _cx: &mut Context<ListState<Self>>) -> Task<()> {
        self.searched = self.servers.iter().filter(|w| w.name.contains(query)).cloned().collect();

        Task::ready(())
    }
}
