use std::{path::Path, sync::Arc};

use bridge::{handle::BackendHandle, message::MessageToBackend};
use gpui::*;
use gpui_component::{button::{Button, ButtonVariants}, checkbox::Checkbox, select::{Select, SelectEvent, SelectState}, sheet::Sheet, spinner::Spinner, tab::{Tab, TabBar}, v_flex, ActiveTheme, IconName, Sizable, ThemeRegistry};
use schema::backend_config::BackendConfig;

use crate::{entity::DataEntities, interface_config::{InterfaceConfig, ThemeMode}};

struct Settings {
    theme_folder: Arc<Path>,
    mode_select: Entity<SelectState<ThemeMode>>,
    backend_handle: BackendHandle,
    pending_request: bool,
    backend_config: Option<BackendConfig>,
    get_configuration_task: Option<Task<()>>,
}

pub fn build_settings_sheet(data: &DataEntities, window: &mut Window, cx: &mut App) -> impl Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static {
    let theme_folder = data.theme_folder.clone();
    let settings = cx.new(|cx| {        

        let mode_select = cx.new(|cx| {
            let mut state = SelectState::new(ThemeMode::default(), Default::default(), window, cx);
            let value = InterfaceConfig::get(cx).theme_mode;
            state.set_selected_value(&value, window, cx);
            state
        });

        cx.subscribe_in(&mode_select, window, |_, entity, _: &SelectEvent<_>, _, cx| {
             let Some(mode) = entity.read(cx).selected_value().cloned() else {
                return;
            };

            InterfaceConfig::get_mut(cx).theme_mode = mode;
            crate::theme_utils::update_theme(cx);
        }).detach();

        let mut settings = Settings {
            theme_folder,
            mode_select,
            backend_handle: data.backend_handle.clone(),
            pending_request: false,
            backend_config: None,
            get_configuration_task: None,
        };

        settings.update_backend_configuration(cx);

        settings
    });

    move |sheet, window, cx| {
        let tab_bar = TabBar::new("bar")
            .prefix(div().w_4())
            .selected_index(0)
            .underline()
            .child(Tab::new().label("Interface"))
            .on_click(|index, window, cx| {
                // todo: switch
            });

        sheet
            .title("Settings")
            .overlay_top(crate::root::sheet_margin_top(window))
            .p_0()
            .child(v_flex()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(tab_bar)
                .child(settings.clone())
            )
    }
}

impl Settings {
    pub fn update_backend_configuration(&mut self, cx: &mut Context<Self>) {
        if self.get_configuration_task.is_some() {
            self.pending_request = true;
            return;
        }

        let (send, recv) = tokio::sync::oneshot::channel();
        self.get_configuration_task = Some(cx.spawn(async move |page, cx| {
            let result: BackendConfig = recv.await.unwrap_or_default();
            let _ = page.update(cx, move |settings, cx| {
                settings.backend_config = Some(result);
                settings.get_configuration_task = None;
                cx.notify();

                if settings.pending_request {
                    settings.pending_request = false;
                    settings.update_backend_configuration(cx);
                }
            });
        }));

        self.backend_handle.send(MessageToBackend::GetBackendConfiguration {
            channel: send,
        });
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let interface_config = InterfaceConfig::get(cx);

        let mut div = v_flex()
            .px_4()
            .py_3()
            .gap_3()
            .child(crate::labelled(
                "Theme Mode",
                Select::new(&self.mode_select)
            ))
            .child(Button::new("open-theme-folder").info().icon(IconName::FolderOpen).label("Open theme folder").on_click({
                let theme_folder = self.theme_folder.clone();
                move |_, window, cx| {
                    crate::open_folder(&theme_folder, window, cx);
                }
            }))
            .child(Button::new("open-theme-repo").info().icon(IconName::Globe).label("Open theme repository").on_click({
                move |_, _, cx| {
                    cx.open_url("https://github.com/longbridge/gpui-component/tree/main/themes");
                }
            }))
            .child(crate::labelled("Deletion",
                v_flex().gap_2()
                    .child(Checkbox::new("confirm-delete-mods")
                        .label("Shift+Click to skip mod delete confirmation")
                        .checked(interface_config.quick_delete_mods)
                        .on_click(|value, _, cx| {
                            InterfaceConfig::get_mut(cx).quick_delete_mods = *value;
                        }))
                    .child(Checkbox::new("confirm-delete-instance")
                        .label("Shift+Click to skip instance delete confirmation")
                        .checked(interface_config.quick_delete_instance).on_click(|value, _, cx| {
                            InterfaceConfig::get_mut(cx).quick_delete_instance = *value;
                        }))
                    )
            );

        if let Some(backend_config) = &self.backend_config {
            div = div
                .child(crate::labelled(
                    "Launching",
                    v_flex().gap_2()
                        .child(Checkbox::new("hide-on-launch")
                            .label("Hide main window on launch")
                            .checked(interface_config.hide_main_window_on_launch)
                            .on_click(|value, _, cx| {
                                InterfaceConfig::get_mut(cx).hide_main_window_on_launch = *value;
                            }))
                        .child(Checkbox::new("open-game-output")
                            .label("Open game output on launch")
                            .checked(backend_config.open_game_output_when_launching)
                            .on_click(cx.listener({
                                let backend_handle = self.backend_handle.clone();
                                move |settings, value, _, cx| {
                                    backend_handle.send(MessageToBackend::SetOpenGameOutputAfterLaunching {
                                        value: *value
                                    });
                                    settings.update_backend_configuration(cx);
                                }
                            })))
                ))
        } else {
            div = div.child(Spinner::new().large());
        }

        div
    }
}
