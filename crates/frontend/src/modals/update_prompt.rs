use std::sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc};

use bridge::{handle::BackendHandle, instance::InstanceID, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, WindowExt, button::{Button, ButtonVariants}, h_flex, input::{Input, InputEvent, InputState}, v_flex
};
use schema::pandora_update::UpdatePrompt;

use crate::ts;

pub fn open_update_prompt(
    update: UpdatePrompt,
    handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let title = ts!("system.update.ask");
    let old_version = ts!("system.update.current", ver = update.old_version);
    let new_version = ts!("system.update.new", ver = update.new_version);

    let size = if update.exe.size < 1000*10 {
        ts!("system.update.size", num = format!("{} bytes", update.exe.size))
    } else if update.exe.size < 1000*1000*10 {
        ts!("system.update.size", num = format!("{}kB", update.exe.size/1000))
    } else if update.exe.size < 1000*1000*1000*10 {
        ts!("system.update.size", num = format!("{}MB", update.exe.size/1000/1000))
    } else {
        ts!("system.update.size", num = format!("{}GB", update.exe.size/1000/1000/1000))
    };

    let size = SharedString::new(size);

    window.open_dialog(cx, move |dialog, _, _| {
        let buttons = h_flex()
            .w_full()
            .gap_2()
            .child(Button::new("update").flex_1().label(ts!("common.update")).success().on_click({
                let handle = handle.clone();
                let update = update.clone();
                move |_, window, cx| {
                    let modal_action = ModalAction::default();
                    handle.send(bridge::message::MessageToBackend::InstallUpdate {
                        update: update.clone(),
                        modal_action: modal_action.clone(),
                    });
                    window.close_all_dialogs(cx);
                    crate::modals::generic::show_notification(window, cx, ts!("system.update.install_error"), modal_action);
                }
            }))
            .child(Button::new("later").flex_1().label(ts!("system.update.later")).on_click(|_, window, cx| {
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
