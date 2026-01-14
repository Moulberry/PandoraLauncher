use std::{path::Path, sync::Arc};

use bridge::{handle::BackendHandle, instance::InstanceID, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent},
    notification::{Notification, NotificationType},
    v_flex,
};
use schema::instance::{InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration, InstanceMemoryConfiguration};

use crate::entity::instance::InstanceEntry;

#[derive(PartialEq, Eq)]
enum NewNameChangeState {
    NoChange,
    InvalidName,
    Pending,
}

pub struct InstanceSettingsSubpage {
    instance: Entity<InstanceEntry>,
    instance_id: InstanceID,
    new_name_input_state: Entity<InputState>,
    memory_override_enabled: bool,
    memory_min_input_state: Entity<InputState>,
    memory_max_input_state: Entity<InputState>,
    jvm_flags_enabled: bool,
    jvm_flags_input_state: Entity<InputState>,
    jvm_binary_enabled: bool,
    jvm_binary_path: Option<Arc<Path>>,
    new_name_change_state: NewNameChangeState,
    backend_handle: BackendHandle,
    _select_file_task: Task<()>,
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

        let entry = instance.read(cx);
        let instance_id = entry.id;

        let memory = entry.configuration.memory.unwrap_or_default();
        let jvm_flags = entry.configuration.jvm_flags.clone().unwrap_or_default();
        let jvm_binary = entry.configuration.jvm_binary.clone().unwrap_or_default();

        let memory_min_input_state = cx.new(|cx| InputState::new(window, cx).default_value(memory.min.to_string()));
        cx.subscribe_in(&memory_min_input_state, window, Self::on_memory_step).detach();
        cx.subscribe(&memory_min_input_state, Self::on_memory_changed).detach();
        let memory_max_input_state = cx.new(|cx| InputState::new(window, cx).default_value(memory.max.to_string()));
        cx.subscribe_in(&memory_max_input_state, window, Self::on_memory_step).detach();
        cx.subscribe(&memory_max_input_state, Self::on_memory_changed).detach();

        let jvm_flags_input_state =
            cx.new(|cx| InputState::new(window, cx).auto_grow(1, 8).default_value(jvm_flags.flags));
        cx.subscribe(&jvm_flags_input_state, Self::on_jvm_flags_changed).detach();

        Self {
            instance: instance.clone(),
            instance_id,
            new_name_input_state,
            memory_override_enabled: memory.enabled,
            memory_min_input_state,
            memory_max_input_state,
            jvm_flags_enabled: jvm_flags.enabled,
            jvm_flags_input_state,
            jvm_binary_enabled: jvm_binary.enabled,
            jvm_binary_path: jvm_binary.path.clone(),
            new_name_change_state: NewNameChangeState::NoChange,
            backend_handle,
            _select_file_task: Task::ready(()),
        }
    }
}

impl InstanceSettingsSubpage {
    pub fn on_new_name_input(&mut self, state: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
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

    pub fn on_memory_step(
        &mut self,
        state: &Entity<InputState>,
        event: &NumberInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            NumberInputEvent::Step(step_action) => match step_action {
                gpui_component::input::StepAction::Decrement => {
                    if let Ok(mut value) = state.read(cx).value().parse::<u32>() {
                        value = value.saturating_div(256).saturating_sub(1).saturating_mul(256).max(128);
                        state.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx);
                        })
                    }
                },
                gpui_component::input::StepAction::Increment => {
                    if let Ok(mut value) = state.read(cx).value().parse::<u32>() {
                        value = value.saturating_div(256).saturating_add(1).saturating_mul(256).max(128);
                        state.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx);
                        })
                    }
                },
            },
        }
    }

    pub fn on_memory_changed(&mut self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        if let InputEvent::Change = event {
            self.backend_handle.send(MessageToBackend::SetInstanceMemory {
                id: self.instance_id,
                memory: self.get_memory_configuration(cx),
            });
        }
    }

    fn get_memory_configuration(&self, cx: &App) -> InstanceMemoryConfiguration {
        let min = self.memory_min_input_state.read(cx).value().parse::<u32>().unwrap_or(0);
        let max = self.memory_max_input_state.read(cx).value().parse::<u32>().unwrap_or(0);

        InstanceMemoryConfiguration {
            enabled: self.memory_override_enabled,
            min,
            max,
        }
    }

    pub fn on_jvm_flags_changed(&mut self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        if let InputEvent::Change = event {
            self.backend_handle.send(MessageToBackend::SetInstanceJvmFlags {
                id: self.instance_id,
                jvm_flags: self.get_jvm_flags_configuration(cx),
            });
        }
    }

    fn get_jvm_flags_configuration(&self, cx: &App) -> InstanceJvmFlagsConfiguration {
        let flags = self.jvm_flags_input_state.read(cx).value();

        InstanceJvmFlagsConfiguration {
            enabled: self.jvm_flags_enabled,
            flags: flags.into(),
        }
    }

    fn get_jvm_binary_configuration(&self) -> InstanceJvmBinaryConfiguration {
        InstanceJvmBinaryConfiguration {
            enabled: self.jvm_binary_enabled,
            path: self.jvm_binary_path.clone(),
        }
    }
}

impl Render for InstanceSettingsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        let header = h_flex().gap_3().mb_1().ml_1().child(div().text_lg().child("Settings"));

        let memory_override_enabled = self.memory_override_enabled;
        let jvm_flags_enabled = self.jvm_flags_enabled;
        let jvm_binary_enabled = self.jvm_binary_enabled;

        let jvm_binary_label = if let Some(path) = &self.jvm_binary_path {
            SharedString::new(path.to_string_lossy())
        } else {
            SharedString::new_static("<unset>")
        };

        let content = v_flex()
            .p_4()
            .gap_4()
            .child(v_flex().child("Instance name").child(
                h_flex().gap_2().child(Input::new(&self.new_name_input_state).w_64()).when(
                    self.new_name_change_state != NewNameChangeState::NoChange,
                    |this| {
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
                    },
                ),
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(Checkbox::new("memory").label("Set Memory").checked(memory_override_enabled).on_click(
                        cx.listener(|page, value, _, cx| {
                            if page.memory_override_enabled != *value {
                                page.memory_override_enabled = *value;
                                page.backend_handle.send(MessageToBackend::SetInstanceMemory {
                                    id: page.instance_id,
                                    memory: page.get_memory_configuration(cx),
                                });
                                cx.notify();
                            }
                        }),
                    ))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                NumberInput::new(&self.memory_min_input_state)
                                    .max_w_64()
                                    .small()
                                    .suffix("MiB")
                                    .disabled(!memory_override_enabled),
                            )
                            .child("Min"),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                NumberInput::new(&self.memory_max_input_state)
                                    .max_w_64()
                                    .small()
                                    .suffix("MiB")
                                    .disabled(!memory_override_enabled),
                            )
                            .child("Max"),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(Checkbox::new("jvm_flags").label("Add JVM Flags").checked(jvm_flags_enabled).on_click(
                        cx.listener(|page, value, _, cx| {
                            if page.jvm_flags_enabled != *value {
                                page.jvm_flags_enabled = *value;
                                page.backend_handle.send(MessageToBackend::SetInstanceJvmFlags {
                                    id: page.instance_id,
                                    jvm_flags: page.get_jvm_flags_configuration(cx),
                                });
                                cx.notify();
                            }
                        }),
                    ))
                    .child(
                        div()
                            .max_w_64()
                            .child(Input::new(&self.jvm_flags_input_state).disabled(!jvm_flags_enabled)),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        Checkbox::new("jvm_binary")
                            .label("Override JVM Binary")
                            .checked(jvm_binary_enabled)
                            .on_click(cx.listener(|page, value, _, cx| {
                                if page.jvm_binary_enabled != *value {
                                    page.jvm_binary_enabled = *value;
                                    page.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                                        id: page.instance_id,
                                        jvm_binary: page.get_jvm_binary_configuration(),
                                    });
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div().max_w_64().child(
                            Button::new("select_jvm_binary")
                                .success()
                                .label(jvm_binary_label)
                                .disabled(!jvm_binary_enabled)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                                        files: true,
                                        directories: false,
                                        multiple: false,
                                        prompt: Some("Select JVM binary".into()),
                                    });

                                    let this_entity = cx.entity();
                                    let add_from_file_task = window.spawn(cx, async move |cx| {
                                        let Ok(result) = receiver.await else {
                                            return;
                                        };
                                        _ = cx.update_window_entity(
                                            &this_entity,
                                            move |this, window, cx| match result {
                                                Ok(Some(paths)) => {
                                                    this.jvm_binary_path = paths.first().map(|v| v.as_path().into());
                                                    this.backend_handle.send(MessageToBackend::SetInstanceJvmBinary {
                                                        id: this.instance_id,
                                                        jvm_binary: this.get_jvm_binary_configuration(),
                                                    });
                                                    cx.notify();
                                                },
                                                Ok(None) => {},
                                                Err(error) => {
                                                    let error = format!("{}", error);
                                                    let notification = Notification::new()
                                                        .autohide(false)
                                                        .with_type(NotificationType::Error)
                                                        .title(error);
                                                    window.push_notification(notification, cx);
                                                },
                                            },
                                        );
                                    });
                                    this._select_file_task = add_from_file_task;
                                })),
                        ),
                    ),
            )
            .child(Button::new("delete").max_w_64().label("Delete this instance").danger().on_click({
                let instance = self.instance.clone();
                let backend_handle = self.backend_handle.clone();
                move |_, window, cx| {
                    let instance = instance.read(cx);
                    let id = instance.id;
                    let name = instance.name.clone();
                    crate::modals::delete_instance::open_delete_instance(id, name, backend_handle.clone(), window, cx);
                }
            }));

        v_flex().p_4().size_full().child(header).child(
            div()
                .size_full()
                .border_1()
                .rounded(theme.radius)
                .border_color(theme.border)
                .child(content),
        )
    }
}
