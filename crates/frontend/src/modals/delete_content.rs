use bridge::{handle::BackendHandle, instance::InstanceID, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

pub fn open_delete_content(
    instance: InstanceID,
    content_ids: Vec<bridge::instance::InstanceContentID>,
    name: SharedString,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_dialog(cx, move |dialog, _, _| {
        let backend_handle = backend_handle.clone();
        let content_ids = content_ids.clone();
        dialog.title(format!("Remove {name}?")).child(
            v_flex().gap_3().child("This will delete the installed file from this instance.").child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(Button::new("cancel").label(t::common::cancel()).on_click(|_, window, cx| {
                        window.close_dialog(cx);
                    }))
                    .child(Button::new("remove").danger().label("Remove").on_click({
                        let backend_handle = backend_handle.clone();
                        let content_ids = content_ids.clone();
                        move |_, window, cx| {
                            backend_handle.send(MessageToBackend::DeleteContent {
                                id: instance,
                                content_ids: content_ids.clone(),
                            });
                            window.close_dialog(cx);
                        }
                    })),
            ),
        )
    });
}
