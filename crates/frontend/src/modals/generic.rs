use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use bridge::modal_action::{ModalAction, ProgressTrackerFinishType};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, WindowExt, button::{Button, ButtonVariant, ButtonVariants}, dialog::DialogTitle, notification::Notification, v_flex
};

use crate::{component::{
    error_alert::ErrorAlert,
    progress_bar::{ProgressBar, ProgressBarColor},
}, icon::PandoraIcon};

pub fn show_notification(
    window: &mut Window,
    cx: &mut App,
    error_title: SharedString,
    modal_action: ModalAction,
) {
    show_notification_with_note(window, cx, error_title, modal_action, Notification::new());
}

pub fn show_notification_with_note(
    window: &mut Window,
    cx: &mut App,
    error_title: SharedString,
    modal_action: ModalAction,
    notification: Notification
) {
    let notify = modal_action.get_notify();
    let task = window.spawn(cx, async move |cx| {
        loop {
            notify.notified().await;
            let res = cx.update_window(cx.window_handle(), |_, window, _| {
                window.refresh();
            });
            if res.is_err() {
                break;
            }
        }
    });

    let notification = notification
        .autohide(false)
        .content(move |notification, window, cx| {
            _ = &task; // Keep refresh task alive

            if let Some(error) = modal_action.get_error_message() {
                let error_widget = ErrorAlert::new(error_title.clone(), error.clone().into());
                return error_widget.into_any_element();
            }

            if modal_action.refcnt() <= 1 || modal_action.get_finished_at().is_some() {
                notification.dismiss(window, cx);
            }

            let (mut progress_entries, needs_animation) = render_progress_trackers(&modal_action, 0.0);
            if needs_animation {
                window.request_animation_frame();
            }

            if let Some(visit_url) = modal_action.get_visit_url() {
                let message = SharedString::new(Arc::clone(&visit_url.message));
                let url = Arc::clone(&visit_url.url);
                progress_entries.push(div().p_3().child(Button::new("visit").success().label(message).on_click(
                    move |_, _, cx| {
                        cx.open_url(&url);
                    },
                )));
            }

            v_flex().gap_2().children(progress_entries).into_any_element()
        });
    window.push_notification(notification, cx);
}

#[derive(Clone)]
struct ModalRoot {
    focus: FocusHandle,
    should_move: Arc<AtomicBool>,
    modal_action: ModalAction,
    title: SharedString,
    error_title: SharedString,
    _notify_task: Arc<Task<()>>,
}

impl Focusable for ModalRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ModalRoot {
    fn render_modal(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let (content, footer, confirm_to_dismiss, opacity) = if let Some(error) = self.modal_action.get_error_message() {
            let error_widget = ErrorAlert::new(self.error_title.clone(), error.clone().into());
            let dismiss = Button::new("ok")
                .label(t::common::ok())
                .on_click(|_, window, _| window.remove_window());

            (
                error_widget.into_any_element(),
                dismiss.into_any_element(),
                true,
                1.0
            )
        } else {
            if self.modal_action.refcnt() <= 1 {
                self.modal_action.set_finished();
            }

            let visit_url = self.modal_action.get_visit_url();

            let mut is_finishing = false;
            let mut modal_opacity = 1.0;
            let mut elapsed_modal = 0.0;
            if let Some(finished_at) = self.modal_action.get_finished_at() {
                is_finishing = true;

                let prevent_finish = visit_url.as_ref().map(|v| v.prevent_auto_finish).unwrap_or(false);

                if !prevent_finish {
                    elapsed_modal = finished_at.elapsed().as_secs_f32();
                    window.request_animation_frame();
                    if elapsed_modal >= 2.0 {
                        window.remove_window();
                        modal_opacity = 0.0;
                    } else if elapsed_modal >= 1.0 {
                        modal_opacity = 2.0 - elapsed_modal;
                    }
                }
            }

            let (mut progress_entries, needs_animation) = render_progress_trackers(&self.modal_action, elapsed_modal);

            if needs_animation {
                window.request_animation_frame();
            }

            if let Some(visit_url) = visit_url {
                let message = SharedString::new(Arc::clone(&visit_url.message));
                let url = Arc::clone(&visit_url.url);
                progress_entries.push(div().p_3().child(Button::new("visit").info().icon(PandoraIcon::Globe).label(message).on_click(
                    move |_, _, cx| {
                        cx.open_url(&url);
                    },
                )));
            }

            let progress = v_flex().gap_2().children(progress_entries);

            if is_finishing {
                if self.modal_action.has_requested_cancel() {
                    window.remove_window();
                }
                let dismiss = Button::new("ok")
                    .with_variant(ButtonVariant::Secondary)
                    .label(t::common::ok())
                    .on_action(move |&crate::Confirm, window, _| window.remove_window())
                    .on_click(|_, window, _| window.remove_window());
                (progress.into_any_element(), dismiss.into_any_element(), true, modal_opacity)
            } else {
                let cancel = self.modal_action.request_cancel.clone();
                let cancel = Button::new("cancel")
                    .disabled(self.modal_action.has_requested_cancel())
                    .label(t::common::cancel())
                    .on_click(move |_, _, _| cancel.cancel());
                (progress.into_any_element(), cancel.into_any_element(), false, modal_opacity)
            }
        };

        let cancel = self.modal_action.request_cancel.clone();
        v_flex()
            .id("root")
            .role(accesskit::Role::Dialog)
            .rounded(theme.radius_lg)
            .bg(theme.tokens.background)
            .border_1()
            .border_color(theme.border)
            .opacity(opacity)
            .min_w(px(448.0))
            .min_h_24()
            .p_4()
            .gap_3()
            .track_focus(&self.focus)
            .on_action(move |&crate::Confirm, window, _| if confirm_to_dismiss {
                window.remove_window();
            } else {
                cancel.cancel();
            })
            .window_control_area(WindowControlArea::Drag)
            .on_any_mouse_down(|_, window, cx| {
                if window.default_prevented() {
                    cx.stop_propagation();
                }
            })
            .on_mouse_down_out({
                let should_move = self.should_move.clone();
                move |_, _, _| {
                    should_move.store(false, Ordering::Relaxed);
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                {
                    let should_move = self.should_move.clone();
                    move |_, _, _| {
                        should_move.store(true, Ordering::Relaxed);
                    }
                },
            )
            .on_mouse_up(
                MouseButton::Left,
                {
                    let should_move = self.should_move.clone();
                    move |_, _, _| {
                        should_move.store(false, Ordering::Relaxed);
                    }
                },
            )
            .on_mouse_move({
                let should_move = self.should_move.clone();
                move |_, window, _| {
                    if should_move.swap(false, Ordering::Relaxed) {
                        window.start_window_move();
                    }
                }
            })
            .child(DialogTitle::new().child(self.title.clone()))
            .child(content)
            .child(footer)
    }
}

fn render_progress_trackers(modal_action: &ModalAction, elapsed_modal: f32) -> (Vec<Div>, bool) {
    modal_action.write_trackers(|trackers| {
        let mut progress_entries = Vec::with_capacity(trackers.len());
        let mut needs_animation = false;

        let mut finishing_tracker_slots = 8;
        trackers.retain(|tracker| {
            if let Some(finished_at) = tracker.get_finished_at() {
                let finish_type = tracker.finish_type();
                if finish_type == ProgressTrackerFinishType::Fast {
                    return false;
                }

                let elapsed = (finished_at.elapsed().as_secs_f32() - elapsed_modal).max(0.0);
                if elapsed >= 2.0 {
                    return false;
                }
            } else {
                finishing_tracker_slots -= 1;
            }
            true
        });

        for tracker in &*trackers {
            let mut opacity = 1.0;

            let mut progress_bar = ProgressBar::new();
            if let Some(progress_amount) = tracker.get_float() {
                progress_bar.amount = progress_amount;
            }

            if let Some(finished_at) = tracker.get_finished_at() {
                if finishing_tracker_slots <= 0 {
                    continue;
                }
                finishing_tracker_slots -= 1;

                let elapsed = finished_at.elapsed().as_secs_f32();
                let elapsed_fade = (elapsed - elapsed_modal).max(0.0);
                if elapsed_fade >= 1.0 {
                    opacity = (2.0 - elapsed_fade).max(0.0);
                }

                let finish_type = tracker.finish_type();
                if finish_type == ProgressTrackerFinishType::Error {
                    progress_bar.color = ProgressBarColor::Error;
                } else {
                    progress_bar.color = ProgressBarColor::Success;
                }
                if elapsed <= 0.5 {
                    progress_bar.color_scale = elapsed * 2.0;
                }

                needs_animation = true;
            }

            let title = tracker.get_title();
            progress_entries.push(div().gap_3().child(SharedString::from(title)).child(progress_bar).opacity(opacity));
        }
        (progress_entries, needs_animation)
    })
}

impl Render for ModalRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.clone()
    }
}

impl IntoElement for ModalRoot {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ModalRoot {
    type RequestLayoutState = ();

    type PrepaintState = AnyElement;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = window.request_layout(Style::default(), [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = cx.theme();
        window.with_text_style(Some(TextStyleRefinement {
            color: Some(theme.foreground),
            font_family: Some(theme.font_family.clone()),
            ..Default::default()
        }), |window| {
            let dialog = self.render_modal(window, cx);
            let mut any = dialog.into_any_element();

            let size = any.layout_as_root(Size::new(AvailableSpace::MinContent, AvailableSpace::MinContent), window, cx);
            if size != window.viewport_size() {
                window.resize(size);
            }

            any.prepaint_at(Point::default(), window, cx);
            any
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        prepaint.paint(window, cx);
    }
}

pub fn show_modal(
    window: &mut Window,
    cx: &mut App,
    title: SharedString,
    error_title: SharedString,
    modal_action: ModalAction,
) {
    let min_size = Size::new(px(448.0), px(96.0));
    let (bounds, display_id) = if let Some(display) = window.display(cx) {
        (display.bounds(), Some(display.id()))
    } else {
        (window.bounds(), None)
    };
    _ = cx.open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: bounds.center() - min_size.center(),
            size: min_size
        })),
        display_id,
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::Floating,
        is_movable: true,
        window_background: WindowBackgroundAppearance::Transparent,
        app_owns_titlebar_drag: true,
        is_resizable: false,
        is_minimizable: false,
        app_id: Some("PandoraLauncher".into()),
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    }, move |window, cx| {
        let notify = modal_action.get_notify();
        let task = window.spawn(cx, async move |cx| {
            loop {
                notify.notified().await;
                let res = cx.update_window(cx.window_handle(), |_, window, _| {
                    window.refresh();
                });
                if res.is_err() {
                    break;
                }
            }
        });


        let focus = cx.focus_handle();

        window.activate_window();
        focus.focus(window, cx);

        // Quickly dismiss window when focus is lost after finishing
        window.on_focus_out(&focus, cx, {
            let modal_action = modal_action.clone();
            move |_, window, _| {
                if modal_action.get_finished_at().is_some() && modal_action.get_error_message().is_none() {
                    window.remove_window();
                }
            }
        }).detach();

        cx.new(|_| ModalRoot {
            focus,
            should_move: Arc::new(AtomicBool::new(false)),
            modal_action,
            title,
            error_title,
            _notify_task: Arc::new(task),
        })
    });
}
