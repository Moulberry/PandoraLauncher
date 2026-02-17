use std::sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc};

use bridge::{handle::BackendHandle, instance::InstanceID, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, WindowExt, button::{Button, ButtonVariants}, h_flex, input::{Input, InputEvent, InputState}, v_flex
};
use schema::pandora_update::UpdatePrompt;

pub fn open_update_prompt(
    update: UpdatePrompt,
    handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let title = SharedString::new_static("Update Pandora?");
    let old_version = SharedString::new(format!("Current version: {}", update.old_version));
    let new_version = SharedString::new(format!("New version: {}", update.new_version));

    let size = if update.exe.size < 1000*10 {
        format!("Update size: {} bytes", update.exe.size)
    } else if update.exe.size < 1000*1000*10 {
        format!("Update size: {}kb", update.exe.size/1000)
    } else if update.exe.size < 1000*1000*1000*10 {
        format!("Update size: {}MB", update.exe.size/1000/1000)
    } else {
        format!("Update size: {}GB", update.exe.size/1000/1000/1000)
    };

    let size = SharedString::new(size);

    window.open_dialog(cx, move |dialog, _, _| {
        let buttons = h_flex()
            .w_full()
            .gap_2()
            .child(Button::new("update").flex_1().label("Update").success().on_click({
                let handle = handle.clone();
                let update = update.clone();
                move |_, window, cx| {
                    let modal_action = ModalAction::default();
                    handle.send(bridge::message::MessageToBackend::InstallUpdate {
                        update: update.clone(),
                        modal_action: modal_action.clone(),
                    });
                    window.close_all_dialogs(cx);
                    crate::modals::generic::show_notification(window, cx, "Unable to install update".into(), modal_action);
                }
            }))
            .child(Button::new("later").flex_1().label("Later").on_click(|_, window, cx| {
                window.close_all_dialogs(cx);
            }));

        dialog
            .title(title.clone())
            .child(v_flex()
                .gap_2()
                .child(v_flex()
                    .child(old_version.clone())
                    .child(new_version.clone())
                    .child(size.clone())
                ).child(buttons))
    });

}
