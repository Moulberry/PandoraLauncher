use bridge::{handle::BackendHandle, instance::InstanceID};
use gpui::{prelude::*, *};
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    notification::NotificationType,
    v_flex,
};

pub fn open_rename_instance(
    instance: InstanceID,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let input_state = cx.new(|cx| InputState::new(window, cx));
    input_state.update(cx, |state, cx| {
        state.set_value(instance_name.clone(), window, cx);
    });

    let current_name = instance_name.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let content = v_flex().gap_4().child(Input::new(&input_state)).child(
            h_flex()
                .gap_2()
                .justify_end()
                .child(Button::new("cancel").label(t::common::cancel()).on_click({
                    move |_, window, cx| {
                        window.close_dialog(cx);
                    }
                }))
                .child(Button::new("rename").label("Rename").success().on_click({
                    let backend_handle = backend_handle.clone();
                    let input_state = input_state.clone();
                    let current_name = current_name.clone();
                    move |_, window, cx| {
                        let new_name = input_state.read(cx).value().trim().to_string();
                        if new_name.is_empty() {
                            window.push_notification((NotificationType::Error, "Instance name cannot be empty"), cx);
                            return;
                        }
                        if new_name.contains('/') || new_name.contains('\\') {
                            window.push_notification(
                                (NotificationType::Error, "Instance name must not contain path separators"),
                                cx,
                            );
                            return;
                        }
                        if new_name != current_name.as_ref() {
                            backend_handle.send(bridge::message::MessageToBackend::RenameInstance {
                                id: instance,
                                name: new_name.into(),
                            });
                        }
                        window.close_dialog(cx);
                    }
                })),
        );

        dialog.title("Rename instance").child(content)
    });
}
