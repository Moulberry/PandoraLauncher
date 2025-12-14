use std::{path::{Path, PathBuf}, sync::{
    atomic::{AtomicUsize, Ordering}, Arc, Mutex
}};

use bridge::{
    handle::BackendHandle, instance::{InstanceID, InstanceModSummary}, message::{LogFiles, MessageToBackend}
};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, h_flex, input::{Input, InputEvent, InputState}, list::{ListDelegate, ListItem, ListState}, select::{Select, SelectEvent, SelectState}, spinner::Spinner, switch::Switch, v_flex, ActiveTheme as _, Icon, IconName, IndexPath, Sizable
};
use rustc_hash::FxHashSet;

use crate::{component::{error_alert::ErrorAlert, named_dropdown::{NamedDropdown, NamedDropdownItem}, readonly_text_field::{ReadonlyTextField, ReadonlyTextFieldWithControls}}, entity::instance::InstanceEntry, png_render_cache, root};

#[derive(PartialEq, Eq)]
enum NewNameChangeState {
    NoChange,
    InvalidName,
    Pending,
}

pub struct InstanceSettingsSubpage {
    instance: Entity<InstanceEntry>,
    new_name_input_state: Entity<InputState>,
    new_name_change_state: NewNameChangeState,
    backend_handle: BackendHandle,
}

impl InstanceSettingsSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let new_name_input_state = cx.new(|cx| InputState::new(window, cx));

        cx.subscribe(&new_name_input_state, Self::on_new_name_input).detach();

        Self {
            instance: instance.clone(),
            new_name_input_state,
            new_name_change_state: NewNameChangeState::NoChange,
            backend_handle,
        }
    }
}

impl InstanceSettingsSubpage {
    pub fn on_new_name_input(
        &mut self,
        state: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            let new_name = state.read(cx).value();
            if new_name.is_empty() {
                self.new_name_change_state = NewNameChangeState::NoChange;
                return;
            }

            let instance = self.instance.read(cx);
            if instance.name == new_name {
                self.new_name_change_state = NewNameChangeState::NoChange;
                return;
            }

            if !crate::is_valid_instance_name(new_name.as_str()) {
                self.new_name_change_state = NewNameChangeState::InvalidName;
                return;
            }

            self.new_name_change_state = NewNameChangeState::Pending;
        }
    }
}

impl Render for InstanceSettingsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        let header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child("Settings"));

        let content = v_flex()
            .p_4()
            .gap_2()
            .child(v_flex()
                .child("Instance name")
                .child(h_flex()
                    .gap_2()
                    .child(Input::new(&self.new_name_input_state).w_64())
                    .when(self.new_name_change_state != NewNameChangeState::NoChange, |this| {
                        if self.new_name_change_state == NewNameChangeState::InvalidName {
                            this.child("Invalid name")
                        } else {
                            this.child(Button::new("setname").label("Update").on_click({
                                let instance = self.instance.clone();
                                let backend_handle = self.backend_handle.clone();
                                let new_name = self.new_name_input_state.read(cx).value();
                                move |_, _, cx| {
                                    let instance = instance.read(cx);
                                    let id = instance.id;
                                    backend_handle.send(MessageToBackend::RenameInstance {
                                        id,
                                        name: new_name.as_str().into(),
                                    });
                                }
                            }))
                        }
                    })
                )
            )
            .child(Button::new("delete").w_64().label("Delete this instance").danger().on_click({
                let instance = self.instance.clone();
                let backend_handle = self.backend_handle.clone();
                move |_, window, cx| {
                    let instance = instance.read(cx);
                    let id = instance.id;
                    let name = instance.name.clone();
                    crate::modals::delete_instance::open_delete_instance(id, name, backend_handle.clone(), window, cx);
                }
            }));

        v_flex()
            .p_4()
            .size_full()
            .child(header)
            .child(div()
                .size_full()
                .border_1()
                .rounded(theme.radius)
                .border_color(theme.border)
                .child(content)
            )
    }
}
