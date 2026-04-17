use std::sync::{Arc, atomic::AtomicBool};

use bridge::{instance::InstanceStatus, message::{BridgeNotificationType, MessageToFrontend}, quit::QuitCoordinator};
use gpui::{AnyWindowHandle, App, AppContext, SharedString, TitlebarOptions, Window, WindowDecorations, WindowOptions, px, size};
use gpui_component::{notification::{Notification, NotificationType}, Root, WindowExt};

use crate::{entity::{DataEntities, account::AccountEntries, instance::InstanceEntries, metadata::FrontendMetadata}, game_output::{GameOutput, GameOutputRoot}, interface_config::InterfaceConfig, root::LauncherRoot};

pub struct Processor {
    data: DataEntities,
    main_window_handle: Option<AnyWindowHandle>,
    main_window_hidden: Arc<AtomicBool>,
    waiting_for_window: Vec<MessageToFrontend>,
    quit_coordinator: QuitCoordinator,
}

impl Processor {
    pub fn new(data: DataEntities, main_window_hidden: Arc<AtomicBool>, quit_coordinator: QuitCoordinator) -> Self {
        Self {
            data,
            main_window_handle: None,
            main_window_hidden,
            waiting_for_window: Vec::new(),
            quit_coordinator,
        }
    }

    pub fn set_main_window_handle(&mut self, window: AnyWindowHandle, cx: &mut App) {
        self.main_window_handle = Some(window);
        self.process_messages_waiting_for_window(cx);
    }

    pub fn process_messages_waiting_for_window(&mut self, cx: &mut App) {
        for message in std::mem::take(&mut self.waiting_for_window) {
            self.process(message, cx);
        }
    }

    #[inline(always)]
    pub fn with_main_window(&mut self, message: MessageToFrontend, cx: &mut App, func: impl FnOnce(&mut Processor, MessageToFrontend, &mut Window, &mut App)) {
        let Some(handle) = self.main_window_handle else {
            self.waiting_for_window.push(message);
            return;
        };

        _ = handle.update(cx, |_, window, cx| {
            (func)(self, message, window, cx);
        });
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
                icon,
                root_path,
                dot_minecraft_folder,
                configuration,
                playtime,
                worlds_state,
                servers_state,
                mods_state,
                resource_packs_state,
            } => {
                InstanceEntries::add(
                    &self.data.instances,
                    id,
                    name.as_str().into(),
                    icon,
                    root_path,
                    dot_minecraft_folder,
                    configuration,
                    playtime,
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
                icon,
                root_path,
                dot_minecraft_folder,
                configuration,
                playtime,
                status,
            } => {
                if status == InstanceStatus::Running {
                    if InterfaceConfig::get(cx).hide_main_window_on_launch {
                        if let Some(handle) = self.main_window_handle.take() {
                            self.main_window_hidden.store(true, std::sync::atomic::Ordering::SeqCst);
                            _ = handle.update(cx, |_, window, _| {
                                window.remove_window();
                            });
                        }
                    }
                } else if status == InstanceStatus::NotRunning {
                    if self.main_window_handle.is_none() && self.main_window_hidden.load(std::sync::atomic::Ordering::SeqCst) {
                        self.quit_coordinator.set_can_quit(false);
                        self.main_window_handle = Some(crate::open_main_window(&self.data, cx));
                        self.main_window_hidden.store(false, std::sync::atomic::Ordering::SeqCst);
                        self.process_messages_waiting_for_window(cx);
                    }
                }

                InstanceEntries::modify(
                    &self.data.instances,
                    id,
                    name.as_str().into(),
                    icon,
                    root_path,
                    dot_minecraft_folder,
                    configuration,
                    playtime,
                    status,
                    cx,
                );
            },
            MessageToFrontend::InstancePlaytimeUpdated { id, playtime } => {
                InstanceEntries::set_playtime(&self.data.instances, id, playtime, cx);
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
            MessageToFrontend::AddNotification { .. } => {
                self.with_main_window(message, cx, |_, message, window, cx| {
                    let MessageToFrontend::AddNotification { notification_type, message } = message else {
                        unreachable!();
                    };

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
                });
            },
            MessageToFrontend::Refresh => {
                let Some(handle) = self.main_window_handle else {
                    return;
                };
                _ = handle.update(cx, |_, window, _| {
                    window.refresh();
                });
            },
            MessageToFrontend::Quit => {
                cx.quit();
            },
            MessageToFrontend::CloseModal => {
                let Some(handle) = self.main_window_handle else {
                    return;
                };
                _ = handle.update(cx, |_, window, cx| {
                    window.close_all_dialogs(cx);
                });
            },
            MessageToFrontend::CreateGameOutputWindow { receiver } => {
                self.quit_coordinator.set_can_quit(false);
                let options = WindowOptions {
                    app_id: Some("PandoraLauncher".into()),
                    window_min_size: Some(size(px(360.0), px(240.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(t::system::game_output().into()),
                        ..Default::default()
                    }),
                    window_decorations: Some(WindowDecorations::Server),
                    ..Default::default()
                };
                _ = cx.open_window(options, |window, cx| {
                    let game_output = cx.new(|cx| GameOutput::new(receiver, cx));
                    let game_output_root = cx.new(|cx| GameOutputRoot::new(game_output.clone(), window, cx));
                    window.activate_window();
                    cx.new(|cx| Root::new(game_output_root, window, cx))
                });
            },
            MessageToFrontend::MoveInstanceToTop { id } => {
                InstanceEntries::move_to_top(&self.data.instances, id, cx);
            },
            MessageToFrontend::MetadataResult { request, result, keep_alive_handle } => {
                FrontendMetadata::set(&self.data.metadata, request, result, keep_alive_handle, cx);
            },
            MessageToFrontend::SkinLibraryUpdated { skin_library } => {
                self.data.set_skin_library(skin_library, cx);
            },
            MessageToFrontend::UpdateAvailable { .. } => {
                self.with_main_window(message, cx, |_, message, window, cx| {
                    let MessageToFrontend::UpdateAvailable { update } = message else {
                        unreachable!();
                    };

                    if let Some(root) = window.root::<Root>().flatten() {
                        if let Ok(launcher_root) = root.read(cx).view().clone().downcast::<LauncherRoot>() {
                            launcher_root.update(cx, |launcher_root, cx| {
                                launcher_root.ui.update(cx, |ui, cx| {
                                    ui.update = Some(update);
                                    cx.notify();
                                });
                            });
                        }
                    }
                });
            },
            MessageToFrontend::OpenOrFocusMainWindow => {
                self.quit_coordinator.set_can_quit(false);

                if let Some(handle) = self.main_window_handle {
                    let res = handle.update(cx, |_, window, _| {
                        window.activate_window();
                    });
                    if res.is_ok() {
                        return;
                    }
                }

                self.main_window_handle = Some(crate::open_main_window(&self.data, cx));
                self.main_window_hidden.store(false, std::sync::atomic::Ordering::SeqCst);
                self.process_messages_waiting_for_window(cx);
            }
        }
    }
}
