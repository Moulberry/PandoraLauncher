use bridge::{handle::BackendHandle, instance::{InstanceContentID, InstanceID}, message::MessageToBackend, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, WindowExt, button::Button, h_flex, v_flex
};

pub fn open_unzip_modpack(
    instance: InstanceID,
    content_id: InstanceContentID,
    content_title: &str,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let title: SharedString = t::instance::content::unzip::title(content_title).into();
    let warning: SharedString = t::instance::content::unzip::warning().into();

    window.open_dialog(cx, move |dialog, _, cx| {
        dialog
            .title(title.clone())
            .line_height(rems(1.2))
            .child(v_flex()
                .w_full()
                .gap_2()
                .child(warning.clone())
                .child(div().text_color(cx.theme().button_danger_foreground).rounded(cx.theme().radius).child(t::common::cannot_be_undone()))
                .child(h_flex()
                    .w_full()
                    .gap_2()
                    .child(Button::new("cancel")
                        .flex_1()
                        .label(t::common::cancel())
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        }))
                    .child(Button::new("unzip")
                        .flex_1()
                        .label(t::instance::content::unzip::action())
                        .on_click({
                            let backend_handle = backend_handle.clone();
                            let title = title.clone();
                            move |_, window, cx| {
                                let modal_action = ModalAction::default();

                                backend_handle.send(MessageToBackend::UnzipModpack {
                                    id: instance,
                                    content_id,
                                    modal_action: modal_action.clone(),
                                });
                                window.close_dialog(cx);

                                crate::modals::generic::show_modal(window, cx, title.clone(), t::instance::content::unzip::error().into(), modal_action);
                            }
                        }))
                ))
    });

}
