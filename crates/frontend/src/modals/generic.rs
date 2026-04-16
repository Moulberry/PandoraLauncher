use std::sync::Arc;

use bridge::modal_action::{ModalAction, ProgressTrackerFinishType};
use gpui::{prelude::*, *};
use gpui_component::{
    WindowExt, button::{Button, ButtonVariant, ButtonVariants}, notification::Notification, v_flex
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
    let notification = notification
        .autohide(false)
        .content(move |notification, window, cx| {
            if let Some(error) = &*modal_action.error.read() {
                let error_widget = ErrorAlert::new(error_title.clone(), error.clone().into());
                return error_widget.into_any_element();
            }

            if modal_action.refcnt() <= 1 || modal_action.get_finished_at().is_some() {
                notification.dismiss(window, cx);
            }

            let mut trackers = modal_action.trackers.trackers.upgradable_read();
            let mut progress_entries = Vec::with_capacity(trackers.len());

            let mut to_remove = Vec::new();

            let mut finishing_tracker_slots = 8;
            for (index, tracker) in trackers.iter().enumerate() {
                if let Some(finished_at) = tracker.get_finished_at() {
                    let finish_type = tracker.finish_type();
                    if finish_type == ProgressTrackerFinishType::Fast {
                        to_remove.push(index);
                        continue;
                    }

                    let elapsed = finished_at.elapsed().as_secs_f32();
                    if elapsed >= 2.0 {
                        to_remove.push(index);
                        continue;
                    }
                } else {
                    finishing_tracker_slots -= 1;
                }
            }

            if !to_remove.is_empty() {
                trackers.with_upgraded(|trackers| {
                    for index in to_remove.iter().rev() {
                        trackers.remove(*index);
                    }
                });
            }

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
                    if elapsed >= 1.0 {
                        opacity = (2.0 - elapsed).max(0.0);
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

                    window.request_animation_frame();
                }

                let title = tracker.get_title();
                progress_entries.push(div().gap_3().child(SharedString::from(title)).child(progress_bar).opacity(opacity));
            }
            drop(trackers);

            if let Some(visit_url) = &*modal_action.visit_url.read() {
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

pub fn show_modal(
    window: &mut Window,
    cx: &mut App,
    title: SharedString,
    error_title: SharedString,
    modal_action: ModalAction,
) {
    window.open_dialog(cx, move |modal, window, cx| {
        if let Some(error) = &*modal_action.error.read() {
            let error_widget = ErrorAlert::new(error_title.clone(), error.clone().into());

            return modal.title(title.clone()).child(v_flex().gap_3().child(error_widget))
                .footer(Button::new("ok").label(t::common::ok()).on_click(|_, window, cx| window.close_dialog(cx)));
        }

        if modal_action.refcnt() <= 1 {
            modal_action.set_finished();
        }

        let mut is_finishing = false;
        let mut modal_opacity = 1.0;
        if let Some(finished_at) = modal_action.get_finished_at() {
            is_finishing = true;

            let prevent_finish = modal_action.visit_url.read().as_ref().map(|v| v.prevent_auto_finish).unwrap_or(false);

            if !prevent_finish {
                let elapsed = finished_at.elapsed().as_secs_f32();
                window.request_animation_frame();
                if elapsed >= 2.0 {
                    window.defer(cx, |window, cx| {
                        window.close_dialog(cx);
                    });
                    return modal.opacity(0.0);
                } else if elapsed >= 1.0 {
                    modal_opacity = 2.0 - elapsed;
                }
            }
        }

        let mut trackers = modal_action.trackers.trackers.upgradable_read();
        let mut progress_entries = Vec::with_capacity(trackers.len());

        let mut to_remove = Vec::new();

        let mut finishing_tracker_slots = 8;
        for (index, tracker) in trackers.iter().enumerate() {
            if let Some(finished_at) = tracker.get_finished_at() {
                let finish_type = tracker.finish_type();
                if finish_type == ProgressTrackerFinishType::Fast {
                    to_remove.push(index);
                    continue;
                }

                let elapsed = finished_at.elapsed().as_secs_f32();
                if elapsed >= 2.0 {
                    to_remove.push(index);
                    continue;
                }
            } else {
                finishing_tracker_slots -= 1;
            }
        }

        if !to_remove.is_empty() {
            trackers.with_upgraded(|trackers| {
                for index in to_remove.iter().rev() {
                    trackers.remove(*index);
                }
            });
        }

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
                if elapsed >= 1.0 {
                    opacity = (2.0 - elapsed).max(0.0);
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

                window.request_animation_frame();
            }

            let title = tracker.get_title();
            progress_entries.push(div().gap_3().child(SharedString::from(title)).child(progress_bar).opacity(opacity));
        }
        drop(trackers);

        if let Some(visit_url) = &*modal_action.visit_url.read() {
            let message = SharedString::new(Arc::clone(&visit_url.message));
            let url = Arc::clone(&visit_url.url);
            progress_entries.push(div().p_3().child(Button::new("visit").info().icon(PandoraIcon::Globe).label(message).on_click(
                move |_, _, cx| {
                    cx.open_url(&url);
                },
            )));
        }

        let progress = v_flex().gap_2().children(progress_entries);

        let request_cancel = modal_action.request_cancel.clone();
        let modal = modal.title(title.clone()).close_button(false).child(progress).opacity(modal_opacity);
        if is_finishing {
            modal
                .footer(Button::new("ok").with_variant(ButtonVariant::Secondary).label(t::common::ok())
                    .on_click(|_, window, cx| window.close_dialog(cx)))
        } else {
            modal
                .overlay_closable(false)
                .keyboard(false)
                .footer(Button::new("cancel").label(t::common::cancel()).on_click(move |_, _, _| request_cancel.cancel()))
        }
    });
}
