use bridge::handle::BackendHandle;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Colorize, InteractiveElementExt, Sizable, button::{Button, ButtonVariants}, h_flex};
use once_cell::sync::Lazy;
use schema::pandora_update::UpdatePrompt;

use crate::{component::page_path::PagePath, icon::PandoraIcon};

#[derive(IntoElement)]
pub struct TitleBar {
    pub page_path: PagePath,
    pub controls: AnyElement,
    pub update: Option<UpdatePrompt>,
    pub send: BackendHandle,
}

#[derive(Default)]
struct TitleBarState {
    should_move: bool,
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state("title-bar-state", cx, |_, _| TitleBarState::default());

        let window_controls = window.window_controls();

        h_flex()
            .id("bar")
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }))
            .when(cfg!(target_os = "linux"), |this| {
                this.on_double_click(|_, window, _| window.zoom_window())
            })
            .when(cfg!(target_os = "macos"), |this| {
                this.on_double_click(|_, window, _| window.titlebar_double_click())
            })
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                if state.should_move {
                    state.should_move = false;
                    window.start_window_move();
                }
            }))
            .w_full()
            .min_h(px(57.0))
            .max_h(px(57.0))
            .h(px(57.0))
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xl()
            .child(h_flex()
                .left_2()
                .w_full()
                .on_any_mouse_down(|_, window, cx| {
                    if window.default_prevented() {
                        cx.stop_propagation();
                    }
                })
                .child(h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(div().overflow_hidden().pr_8().child(self.page_path))
                    .child(div().flex_1().child(self.controls))
                )
                .when(!cfg!(target_os = "macos"), |this| {
                    this.child(h_flex()
                        .flex_shrink_0()
                        .h_full()
                        .gap_1()
                        .on_any_mouse_down(|_, window, cx| {
                            if window.default_prevented() {
                                cx.stop_propagation();
                            }
                        })
                        .when_some(self.update, |this, update| {
                            this.child(Button::new("update")
                                .label("Update Available")
                                .success()
                                .compact()
                                .small()
                                .ml_2()
                                .icon(PandoraIcon::Download)
                                .on_click({
                                    let send = self.send.clone();
                                    move |_, window, cx| {
                                        crate::modals::update_prompt::open_update_prompt(update.clone(), send.clone(), window, cx);
                                    }
                                })
                            )
                        })
                        .when(!cfg!(target_os = "macos"), |this| {
                            this
                                .when(window_controls.minimize, |this| this.child(WindowControl::Minimize))
                                .when(window_controls.maximize, |this| this.child(if window.is_maximized() {
                                    WindowControl::Restore
                                } else {
                                    WindowControl::Maximize
                                }))
                                .child(WindowControl::Close)
                        }))
                }))
    }
}

#[derive(IntoElement, Clone, Copy, PartialEq, Eq)]
pub enum WindowControl {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl RenderOnce for WindowControl {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = h_flex()
            .id(match self {
                WindowControl::Minimize => "minimize",
                WindowControl::Maximize => "maximize",
                WindowControl::Restore => "restore",
                WindowControl::Close => "close",
            })
            .occlude()
            .window_control_area(match self {
                WindowControl::Minimize => WindowControlArea::Min,
                WindowControl::Maximize | WindowControl::Restore => WindowControlArea::Max,
                WindowControl::Close => WindowControlArea::Close,
            })
            .size_8()
            .justify_center()
            .content_center()
            .rounded(cx.theme().radius)
            .hover(|this| {
                let col = if self == WindowControl::Close {
                    cx.theme().danger_hover
                } else if cx.theme().mode.is_dark() {
                    cx.theme().secondary.lighten(0.1).opacity(0.8)
                } else {
                    cx.theme().secondary.darken(0.1).opacity(0.8)
                };
                this.bg(col)
            });
        if cfg!(windows) {
            base
                .font_family(*ICON_FONT)
                .text_size(px(10.0))
                .child(match self {
                    WindowControl::Minimize => "\u{e921}",
                    WindowControl::Maximize => "\u{e922}",
                    WindowControl::Restore => "\u{e923}",
                    WindowControl::Close => "\u{e8bb}",
                })
        } else {
            base
                .on_click(move |_, window, _| {
                    match self {
                        WindowControl::Minimize => window.minimize_window(),
                        WindowControl::Maximize | WindowControl::Restore => window.zoom_window(),
                        WindowControl::Close => window.remove_window(),
                    }
                }).child(match self {
                    WindowControl::Minimize => PandoraIcon::WindowMinimize,
                    WindowControl::Maximize => PandoraIcon::WindowMaximize,
                    WindowControl::Restore => PandoraIcon::WindowRestore,
                    WindowControl::Close => PandoraIcon::WindowClose,
                })
        }
    }
}


#[cfg(not(windows))]
static ICON_FONT: Lazy<&'static str> = Lazy::new(|| "Segoe MDL2 Assets");

#[cfg(windows)]
static ICON_FONT: Lazy<&'static str> = Lazy::new(|| {
    let mut version = unsafe { std::mem::zeroed() };
    let status = unsafe {
        windows::Wdk::System::SystemServices::RtlGetVersion(&mut version)
    };

    if status.is_ok() && version.dwBuildNumber >= 22000 {
        // Windows 11
        "Segoe Fluent Icons"
    } else {
        // Windows 10 and prior
        "Segoe MDL2 Assets"
    }
});
