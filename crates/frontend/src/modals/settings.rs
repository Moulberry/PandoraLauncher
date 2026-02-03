use std::{path::Path, sync::Arc};

use bridge::{handle::BackendHandle, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{button::{Button, ButtonVariants}, checkbox::Checkbox, input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction}, select::{SearchableVec, Select, SelectEvent, SelectState}, sheet::Sheet, spinner::Spinner, tab::{Tab, TabBar}, v_flex, h_flex, ActiveTheme, IconName, Sizable, ThemeRegistry};
use schema::backend_config::BackendConfig;

use crate::{entity::DataEntities, interface_config::InterfaceConfig};

struct Settings {
    theme_folder: Arc<Path>,
    theme_select: Entity<SelectState<SearchableVec<SharedString>>>,
    backend_handle: BackendHandle,
    pending_request: bool,
    backend_config: Option<BackendConfig>,
    get_configuration_task: Option<Task<()>>,
    global_memory_max_state: Entity<InputState>,
    global_jvm_args_state: Entity<InputState>,
    pub active_tab: usize,
    window_handle: AnyWindowHandle,
}

pub fn build_settings_sheet(data: &DataEntities, window: &mut Window, cx: &mut App) -> impl Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static {
    let theme_folder = data.theme_folder.clone();
    let settings = cx.new(|cx| {        

        let theme_select = cx.new(|cx| {
            let mut themes: Vec<SharedString> = ThemeRegistry::global(cx)
                .sorted_themes()
                .iter()
                .map(|cfg| cfg.name.clone())
                .collect();
            themes.insert(0, crate::theme_utils::SYSTEM_DEFAULT_THEME.into());
            let delegate = SearchableVec::new(themes);
            let mut state = SelectState::new(
                delegate,
                None,
                window,
                cx,
            );
            state.set_selected_value(
                &InterfaceConfig::get(cx).active_theme.clone(),
                window,
                cx,
            );
            state
        });

        cx.subscribe_in(&theme_select, window, |_, entity, _: &SelectEvent<_>, _, cx| {
             let Some(theme) = entity.read(cx).selected_value().cloned() else {
                return;
            };

            InterfaceConfig::get_mut(cx).active_theme = theme;
            crate::theme_utils::update_theme(cx);
        })
        .detach();
        
        let config = InterfaceConfig::get(cx);
        let global_jvm_args = config.global_jvm_args.clone().unwrap_or_default();

        let global_memory_max_state = cx.new(|cx| InputState::new(window, cx).default_value("0".to_string()));
        let backend_handle_for_debounce = data.backend_handle.clone();
        cx.subscribe_in(&global_memory_max_state, window, move |_, entity, event: &NumberInputEvent, window, cx: &mut Context<Settings>| {
            if let NumberInputEvent::Step(action) = event {
                let current = entity.read(cx).value().parse::<u32>().unwrap_or(0);
                let new_value = match action {
                    StepAction::Increment => current.saturating_add(512).min(32768),
                    StepAction::Decrement => current.saturating_sub(512).max(0),
                };
                entity.update(cx, |input, cx| {
                    input.set_value(new_value.to_string(), window, cx);
                });
                let value = if new_value == 0 { None } else { Some(new_value) };
                
                // Send immediately to backend with detached task
                let backend_handle = backend_handle_for_debounce.clone();
                cx.background_executor().spawn(async move {
                    backend_handle.send(MessageToBackend::SetGlobalMemoryMax { value });
                }).detach();
            }
        })
        .detach();
        
        let backend_handle_for_input = data.backend_handle.clone();
        cx.subscribe(&global_memory_max_state, move |_, entity, event: &InputEvent, cx: &mut Context<Settings>| {
             if let InputEvent::Change = event {
                 let value = entity.read(cx).value().parse::<u32>().ok().filter(|&v| v > 0);
                 
                 // Debounce with detached task that won't be dropped
                 let backend_handle = backend_handle_for_input.clone();
                 cx.background_executor().spawn(async move {
                     gpui::Timer::after(std::time::Duration::from_millis(500)).await;
                     backend_handle.send(MessageToBackend::SetGlobalMemoryMax { value });
                 }).detach();
             }
        })
        .detach();

        // global_java_path was removed - it was never used

        let global_jvm_args_state = cx.new(|cx| InputState::new(window, cx).default_value(global_jvm_args));
        cx.subscribe(&global_jvm_args_state, |_, entity, event: &InputEvent, cx: &mut Context<Settings>| {
            if let InputEvent::Change = event {
                let value = entity.read(cx).value();
                let mut config = InterfaceConfig::get_mut(cx);
                config.global_jvm_args = if value.is_empty() { None } else { Some(value.to_string()) };
            }
        })
        .detach();

        let mut settings = Settings {
            theme_folder,
            theme_select,
            backend_handle: data.backend_handle.clone(),
            pending_request: false,
            backend_config: None,
            get_configuration_task: None,
            global_memory_max_state,
            global_jvm_args_state,
            active_tab: 0,
            window_handle: window.window_handle().into(),
        };

        settings.update_backend_configuration(cx);

        settings
    });

    move |sheet, window, cx| {
        let active_tab = settings.read(cx).active_tab;
        let tab_bar = TabBar::new("bar")
            .prefix(div().w_4())
            .selected_index(active_tab)
            .underline()
            .child(Tab::new().label("Interface"))
            .child(Tab::new().label("Launcher"))
            .on_click({
                let settings = settings.clone();
                move |index, _, cx| {
                    settings.update(cx, |settings, cx| {
                        settings.active_tab = *index;
                        cx.notify();
                    });
                }
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
                settings.backend_config = Some(result.clone());
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

        let mut div = v_flex().px_4().py_3().gap_3();

        if self.active_tab == 0 {
            div = div
                .child(crate::labelled("Theme", Select::new(&self.theme_select)))
                .child(Button::new("open-theme-folder").info().icon(IconName::FolderOpen).label("Open theme folder").on_click({
                    let theme_folder = self.theme_folder.clone();
                    move |_, window, cx| { crate::open_folder(&theme_folder, window, cx); }
                }))
                .child(Button::new("open-theme-repo").info().icon(IconName::Globe).label("Open theme repository").on_click({
                    move |_, _, cx| { cx.open_url("https://github.com/longbridge/gpui-component/tree/main/themes"); }
                }))
                .child(crate::labelled("Deletion",
                    v_flex().gap_2()
                        .child(Checkbox::new("confirm-delete-mods")
                            .label("Shift+Click to skip mod delete confirmation")
                            .checked(interface_config.quick_delete_mods)
                            .on_click(|value, _, cx| { InterfaceConfig::get_mut(cx).quick_delete_mods = *value; }))
                        .child(Checkbox::new("confirm-delete-instance")
                            .label("Shift+Click to skip instance delete confirmation")
                            .checked(interface_config.quick_delete_instance).on_click(|value, _, cx| { InterfaceConfig::get_mut(cx).quick_delete_instance = *value; }))
                ));
        } else if self.active_tab == 1 {
             div = div.child(crate::labelled("Global Launcher Settings",
                v_flex().gap_2()
                    .child(h_flex().gap_2().items_center().child(gpui::div().w_32().child("Memory")).child(NumberInput::new(&self.global_memory_max_state).suffix("MB")))
                    .child(h_flex().gap_2().items_center().child(gpui::div().w_32().child("JVM Args")).child(Input::new(&self.global_jvm_args_state)))
            ));

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
                                .checked(!backend_config.dont_open_game_output_when_launching)
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
        }

        div
    }
}
