
use bridge::{handle::BackendHandle, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    WindowExt, button::{Button, ButtonVariants}, h_flex, v_flex
};
use schema::pandora_update::UpdatePrompt;


pub fn open_update_prompt(
    update: UpdatePrompt,
    handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let title = t::system::update::title();
    let old_version: SharedString = t::system::update::current(&update.old_version).into();
    let new_version: SharedString = t::system::update::new(&update.new_version).into();

    let size = if update.exe.size < 1000*10 {
        t::system::update::size::bytes(update.exe.size)
    } else if update.exe.size < 1000*1000*10 {
        t::system::update::size::kb(update.exe.size/1000)
    } else if update.exe.size < 1000*1000*1000*10 {
        t::system::update::size::mb(update.exe.size/1000/1000)
    } else {
        t::system::update::size::gb(update.exe.size/1000/1000/1000)
    };

    let size = SharedString::from(size);

    window.open_dialog(cx, move |dialog, _, _| {
        let buttons = h_flex()
            .w_full()
            .gap_2()
            .child(Button::new("update").flex_1().label(t::common::update()).success().on_click({
                let handle = handle.clone();
                let update = update.clone();
                move |_, window, cx| {
                    let modal_action = ModalAction::default();
                    handle.send(bridge::message::MessageToBackend::InstallUpdate {
                        update: update.clone(),
                        modal_action: modal_action.clone(),
                    });
                    window.close_all_dialogs(cx);
                    crate::modals::generic::show_notification(window, cx, t::system::update::install_error().into(), modal_action);
                }
            }))
            .child(Button::new("later").flex_1().label(t::system::update::later()).on_click(|_, window, cx| {
                window.close_all_dialogs(cx);
            }));

        dialog
            .title(title)
            .overlay_closable(false)
            .child(v_flex()
                .gap_2()
                .child(v_flex()
                    .child(old_version.clone())
                    .child(new_version.clone())
                    .child(size.clone())
                ).child(buttons))
    });

}
