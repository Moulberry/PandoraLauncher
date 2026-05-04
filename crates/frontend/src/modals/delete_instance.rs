use std::sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc};

use bridge::{handle::BackendHandle, instance::InstanceID};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, input::{Input, InputEvent, InputState}, v_flex, Disableable, WindowExt
};

pub fn open_delete_instance(
    instance: InstanceID,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let stage = Arc::new(AtomicU8::new(0));
    let correct_name = Arc::new(AtomicBool::new(false));

    let title: SharedString = t::instance::delete_dialog::title(&instance_name).into();
    let warning_message: SharedString = t::instance::delete_dialog::warning(&instance_name).into();
    let confirm_message: SharedString = t::instance::delete_dialog::confirm_text(&instance_name).into();

    let input_state = cx.new(|cx| InputState::new(window, cx));

    let correct_name2 = correct_name.clone();
    let instance_name2 = instance_name.clone();
    let _input_subscription = cx.subscribe(&input_state, move |state, event: &InputEvent, cx| {
        if let InputEvent::Change = event {
            let value = state.read(cx).value();
            correct_name2.store(value == instance_name2, Ordering::Relaxed);
        }
    });

    window.open_dialog(cx, move |dialog, _, _| {
        let _ = &_input_subscription;

        let stage_val = stage.load(Ordering::Relaxed);
        let correct = correct_name.load(Ordering::Relaxed);

        let content = match stage_val {
            0 => {
                v_flex()
                    .child(Button::new("delete").label(t::instance::delete_dialog::label()).on_click({
                        let stage = stage.clone();
                        move |_, _, _| {
                            stage.store(1, Ordering::Relaxed);
                        }
                    }))
            }
            1 => {
                v_flex()
                    .gap_2()
                    .child(warning_message.clone())
                    .child(Button::new("confirm").label(t::instance::delete_dialog::check()).on_click({
                        let stage = stage.clone();
                        let input_state = input_state.clone();
                        move |_, window, cx| {
                            input_state.update(cx, |input_state, cx| {
                                input_state.focus(window, cx);
                            });
                            stage.store(2, Ordering::Relaxed);
                        }
                    }))
            }
            2 => {
                // .div() and .child(div().h_2()) are workarounds for a weird layout bug
                // where the Input would be set to its minimum width when confirm_message wrapped
                div()
                    .child(confirm_message.clone())
                    .child(div().h_2())
                    .child(Input::new(&input_state).border_color(gpui::red()))
                    .child(div().h_2())
                    .child(Button::new("confirm").label(t::instance::delete()).danger().disabled(!correct).on_click({
                        let backend_handle = backend_handle.clone();
                        move |_, window, cx| {
                            backend_handle.send(bridge::message::MessageToBackend::DeleteInstance {
                                id: instance
                            });
                            window.close_all_dialogs(cx);
                        }
                    }))
            }
            _ => {
                unreachable!()
            }
        };

        dialog
            .on_ok({
                let backend_handle = backend_handle.clone();
                move |_, _, _| {
                    if stage_val != 2 || !correct {
                        return false;
                    }
                    backend_handle.send(bridge::message::MessageToBackend::DeleteInstance {
                        id: instance
                    });
                    true
                }
            })
            .title(title.clone())
            .child(content)
    });

}
