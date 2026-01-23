use std::collections::HashMap;

use bridge::message::{BridgeNotificationType, MessageToFrontend};
use gpui::{px, size, AnyWindowHandle, App, AppContext, Entity, SharedString, TitlebarOptions, WindowDecorations, WindowHandle, WindowOptions};
use gpui_component::{notification::{Notification, NotificationType}, Root, WindowExt};

use crate::{entity::{account::AccountEntries, instance::InstanceEntries, metadata::FrontendMetadata, DataEntities}, game_output::{GameOutput, GameOutputRoot}};

pub struct Processor {
    data: DataEntities,
    game_output_windows: HashMap<usize, (WindowHandle<Root>, Entity<GameOutput>)>,
    main_window_handle: AnyWindowHandle
}

impl Processor {
    pub fn new(data: DataEntities, main_window_handle: AnyWindowHandle) -> Self {
        Self {
            data,
            game_output_windows: HashMap::new(),
            main_window_handle
        }
    }

    pub fn process(&mut self, message: MessageToFrontend, cx: &mut App) {
        match message {
            MessageToFrontend::AccountsUpdated {
                accounts,
                selected_account,
            } => {
                AccountEntries::set(&self.data.accounts, accounts, selected_account, cx);
            },
            MessageToFrontend::InstanceAdded {
                id,
                name,
                dot_minecraft_folder,
                configuration,
                worlds_state,
                servers_state,
                mods_state,
                resource_packs_state,
            } => {
                InstanceEntries::add(
                    &self.data.instances,
                    id,
                    name.as_str().into(),
                    dot_minecraft_folder,
                    configuration,
                    worlds_state,
                    servers_state,
                    mods_state,
                    resource_packs_state,
                    cx,
                );
            },
            MessageToFrontend::InstanceRemoved { id } => {
                InstanceEntries::remove(&self.data.instances, id, cx);
            },
            MessageToFrontend::InstanceModified {
                id,
                name,
                dot_minecraft_folder,
                configuration,
                status,
            } => {
                InstanceEntries::modify(
                    &self.data.instances,
                    id,
                    name.as_str().into(),
                    dot_minecraft_folder,
                    configuration,
                    status,
                    cx,
                );
            },
            MessageToFrontend::InstanceWorldsUpdated { id, worlds } => {
                InstanceEntries::set_worlds(&self.data.instances, id, worlds, cx);
            },
            MessageToFrontend::InstanceServersUpdated { id, servers } => {
                InstanceEntries::set_servers(&self.data.instances, id, servers, cx);
            },
            MessageToFrontend::InstanceModsUpdated { id, mods } => {
                InstanceEntries::set_mods(&self.data.instances, id, mods, cx);
            },
            MessageToFrontend::InstanceResourcePacksUpdated { id, resource_packs } => {
                InstanceEntries::set_resource_packs(&self.data.instances, id, resource_packs, cx);
            },
            MessageToFrontend::AddNotification { notification_type, message } => {
                self.main_window_handle.update(cx, |_, window, cx| {
                    let notification_type = match notification_type {
                        BridgeNotificationType::Success => NotificationType::Success,
                        BridgeNotificationType::Info => NotificationType::Info,
                        BridgeNotificationType::Error => NotificationType::Error,
                        BridgeNotificationType::Warning => NotificationType::Warning,
                    };
                    let mut notification: Notification = (notification_type, SharedString::from(message)).into();
                    if let NotificationType::Error = notification_type {
                        notification = notification.autohide(false);
                    }
                    window.push_notification(notification, cx);
                }).unwrap();
            },
            MessageToFrontend::Refresh => {
                _ = self.main_window_handle.update(cx, |_, window, _| {
                    window.refresh();
                });
            },
            MessageToFrontend::CloseModal => {
                _ = self.main_window_handle.update(cx, |_, window, cx| {
                    window.close_all_dialogs(cx);
                });
            },
            MessageToFrontend::CreateGameOutputWindow { id, keep_alive } => {
                let options = WindowOptions {
                    app_id: Some("PandoraLauncher".into()),
                    window_min_size: Some(size(px(360.0), px(240.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::new_static("Minecraft Game Output")),
                        ..Default::default()
                    }),
                    window_decorations: Some(WindowDecorations::Server),
                    ..Default::default()
                };
                _ = cx.open_window(options, |window, cx| {
                    let game_output = cx.new(|_| GameOutput::default());
                    let game_output_root = cx
                        .new(|cx| GameOutputRoot::new(keep_alive, game_output.clone(), window, cx));
                    window.activate_window();
                    let window_handle = window.window_handle().downcast::<Root>().unwrap();
                    self.game_output_windows.insert(id, (window_handle, game_output.clone()));
                    cx.new(|cx| Root::new(game_output_root, window, cx))
                });
            },
            MessageToFrontend::AddGameOutput {
                id,
                time,
                level,
                text,
            } => {
                if let Some((window, game_output)) = self.game_output_windows.get(&id) {
                    _ = window.update(cx, |_, window, cx| {
                        game_output.update(cx, |game_output, _| {
                            game_output.add(time, level, text);
                        });
                        window.refresh();
                    });
                }
            },
            MessageToFrontend::MoveInstanceToTop { id } => {
                InstanceEntries::move_to_top(&self.data.instances, id, cx);
            },
            MessageToFrontend::MetadataResult { request, result, keep_alive_handle } => {
                FrontendMetadata::set(&self.data.metadata, request, result, keep_alive_handle, cx);
            },
        }
    }
}
