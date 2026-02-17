use std::{
    ffi::OsString,
    sync::{Arc, atomic::Ordering},
};

use bridge::{
    handle::BackendHandle,
    instance::{InstanceID, InstanceServerSummary, InstanceWorldSummary},
    message::{AtomicBridgeDataLoadState, MessageToBackend, QuickPlayLaunch}, serial::AtomicOptionSerial,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Icon, IndexPath,
    button::{Button, ButtonVariants},
    h_flex,
    list::{ListDelegate, ListItem, ListState},
    v_flex,
};

use crate::{entity::instance::InstanceEntry, png_render_cache, root, ts};

pub struct InstanceQuickplaySubpage {
    instance: InstanceID,
    backend_handle: BackendHandle,
    worlds_state: Arc<AtomicBridgeDataLoadState>,
    world_list: Entity<ListState<WorldsListDelegate>>,
    servers_state: Arc<AtomicBridgeDataLoadState>,
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
        let instance = instance.read(cx);
        let instance_id = instance.id;

        let worlds_state = Arc::clone(&instance.worlds_state);
        let servers_state = Arc::clone(&instance.servers_state);

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
            }).detach();

            ListState::new(worlds_list_delegate, window2, cx).selectable(false).searchable(true)
        });

        let server_list = cx.new(move |cx| {
            cx.observe(&servers, |list: &mut ListState<ServersListDelegate>, servers, cx| {
                let servers = servers.read(cx).to_vec();
                let delegate = list.delegate_mut();
                delegate.servers = servers.clone();
                delegate.searched = servers;
                cx.notify();
            }).detach();

            ListState::new(servers_list_delegate, window, cx).selectable(false).searchable(true)
        });

        Self {
            instance: instance_id,
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

        let state = self.worlds_state.load(Ordering::SeqCst);
        if state.should_send_load_request() {
            self.backend_handle.send_with_serial(MessageToBackend::RequestLoadWorlds { id: self.instance }, &self.worlds_serial);
        }

        let state = self.servers_state.load(Ordering::SeqCst);
        if state.should_send_load_request() {
            self.backend_handle.send_with_serial(MessageToBackend::RequestLoadServers { id: self.instance }, &self.servers_serial);
        }

        let worlds_header = div().mb_1().ml_1().text_lg().child(ts!("instance.worlds"));
        let servers_header = div().mb_1().ml_1().text_lg().child(ts!("instance.servers"));

        v_flex().p_4().gap_4().size_full().child(
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

    fn render_item(&mut self, ix: IndexPath, _window: &mut Window, cx: &mut Context<ListState<Self>>) -> Option<Self::Item> {
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

        let play_icon = Icon::empty().path("icons/play.svg");

        let id = self.id;
        let name = self.name.clone();
        let backend_handle = self.backend_handle.clone();
        let target = summary.level_path.file_name().unwrap().to_owned();
        let item = ListItem::new(ix).p_1().child(
            h_flex()
                .gap_1()
                .child(
                    div()
                        .child(Button::new(ix).success().icon(play_icon).on_click(move |_, window, cx| {
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

    fn render_item(&mut self, ix: IndexPath, _window: &mut Window, cx: &mut Context<ListState<Self>>) -> Option<Self::Item> {
        let summary = self.searched.get(ix.row)?;

        let icon = if let Some(png_icon) = summary.png_icon.as_ref() {
            png_render_cache::render(Arc::clone(png_icon), cx)
        } else {
            gpui::img(ImageSource::Resource(Resource::Embedded("images/default_world.png".into())))
        };

        let description = v_flex()
            .child(SharedString::from(summary.name.clone()))
            .child(div().text_color(Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0}).child(SharedString::from(summary.ip.clone())));

        let play_icon = Icon::empty().path("icons/play.svg");

        let id = self.id;
        let name = self.name.clone();
        let backend_handle = self.backend_handle.clone();
        let target = OsString::from(summary.ip.to_string());
        let item = ListItem::new(ix).p_1().child(
            h_flex()
                .gap_1()
                .child(
                    div()
                        .child(Button::new(ix).success().icon(play_icon).on_click(move |_, window, cx| {
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
