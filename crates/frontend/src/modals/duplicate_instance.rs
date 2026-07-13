use std::sync::Arc;

use bridge::{handle::BackendHandle, instance::InstanceID, message::MessageToBackend, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, WindowExt, button::Button, dialog::Dialog, h_flex, input::{Input, InputEvent, InputState}, v_flex};

use crate::{entity::instance::InstanceEntries, get_unique_instance_name, is_valid_instance_name, modals::generic};

struct DuplicateInstanceModalState {
    instance_id: InstanceID,
    backend_handle: BackendHandle,
    name_input_state: Entity<InputState>,
    name_invalid: bool,
    default_name: SharedString,
    _name_input_subscription: Subscription,
}

impl DuplicateInstanceModalState {
    pub fn new(
        instance_id: InstanceID,
        instance_name: SharedString,
        instances: Entity<InstanceEntries>,
        backend_handle: BackendHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let instance_names: Arc<[SharedString]> =
            instances.read(cx).entries.iter().map(|(_, v)| v.read(cx).name.clone()).collect();

        let instance_name_strings: Vec<&str> = instance_names.iter().map(|s| s.as_str()).collect();
        let default_name = SharedString::from(get_unique_instance_name(&t::instance::duplicate::copy_of(&instance_name), &instance_name_strings));

        let name_input_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(default_name.clone())
        });

        let _name_input_subscription = {
            let instance_names = Arc::clone(&instance_names);
            cx.subscribe_in(&name_input_state, window, move |this, input_state, _: &InputEvent, _, cx| {
                let text = input_state.read(cx).value();
                let resolved = if text.is_empty() {
                    this.default_name.as_str()
                } else {
                    text.as_str()
                };
                if resolved.is_empty() || !is_valid_instance_name(resolved) {
                    this.name_invalid = true;
                    return;
                }
                this.name_invalid = instance_names.contains(&text);
            })
        };

        Self {
            instance_id,
            backend_handle,
            name_input_state,
            name_invalid: false,
            default_name,
            _name_input_subscription,
        }
    }

    pub fn render(&mut self, dialog: Dialog, _window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        let content = v_flex()
            .gap_3()
            .child(crate::labelled(
                t::instance::name(),
                Input::new(&self.name_input_state).when(self.name_invalid, |this| this.border_color(cx.theme().danger)),
            ));

        let name_is_invalid = self.name_invalid;
        dialog
            .overlay_closable(false)
            .title(t::instance::duplicate::title())
            .child(content)
            .when(name_is_invalid, |dialog| {
                dialog.footer(h_flex().gap_2().w_full()
                    .child(Button::new("cancel").flex_1().label(t::common::cancel())
                        .on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("ok").flex_1().opacity(0.5).label(t::common::ok())))
            })
            .when(!name_is_invalid, |dialog| {
                dialog.footer(h_flex().gap_2().w_full()
                    .child(Button::new("cancel").flex_1().label(t::common::cancel())
                        .on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("ok").flex_1().label(t::common::ok())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let mut name = this.name_input_state.read(cx).value().clone();
                            if name.is_empty() {
                                name = this.default_name.clone();
                            }

                            let backend_handle = this.backend_handle.clone();
                            let instance_id = this.instance_id;
                            let modal_action = ModalAction::default();

                            window.close_dialog(cx);

                            generic::show_modal(window, cx, t::instance::duplicate::progress().into(), t::instance::duplicate::error().into(), modal_action.clone());

                            backend_handle.send(MessageToBackend::DuplicateInstance {
                                id: instance_id,
                                name: name.as_str().into(),
                                modal_action,
                            });
                        }))))
            })
    }
}

pub fn open_duplicate_instance(
    instance_id: InstanceID,
    instance_name: SharedString,
    instances: Entity<InstanceEntries>,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let state = cx.new(|cx| {
        DuplicateInstanceModalState::new(instance_id, instance_name, instances, backend_handle, window, cx)
    });

    window.open_dialog(cx, move |modal, window, cx| {
        cx.update_entity(&state, |state, cx| {
            state.render(modal, window, cx)
        })
    });
}
