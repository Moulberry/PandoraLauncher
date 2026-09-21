use std::rc::Rc;

use gpui::*;
use gpui_component::{
    Disableable, FocusableExt,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    switch::Switch,
};
use schema::instance::InstanceJvmBinaryConfiguration;

use crate::{
    component::path_label::PathLabel,
    settings::{SettingGroup, SettingItem, SettingItemWidget, SettingPage},
};

pub(super) fn create_page() -> SettingPage {
    SettingPage {
        title: t::settings::java,
        groups: vec![
            SettingGroup {
                title: Some(t::settings::java::defaults),
                items: vec![
                    SettingItem {
                        title: t::settings::java::defaults::enable_memory,
                        description: t::settings::java::defaults::enable_memory_desc,
                        widget: SettingItemWidget::Backend(Rc::new(|backend, _, cx| {
                            let enabled = backend.memory.as_ref().is_some_and(|memory| memory.enabled);
                            Switch::new("enable-default-memory").checked(enabled).on_click(cx.listener(|root, val, _, cx| {
                                let Some(backend) = root.backend_config().cloned() else {
                                    return;
                                };
                                let mut memory = backend.memory.unwrap_or_default();
                                if memory.enabled == *val {
                                    return;
                                }
                                memory.enabled = *val;
                                root.set_launch_defaults(
                                    Some(memory),
                                    backend.jvm_flags.clone(),
                                    backend.jvm_binary.clone(),
                                    cx,
                                );
                            })).into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::java::defaults::memory_min,
                        description: t::settings::java::defaults::memory_min_desc,
                        widget: create_memory_min_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::java::defaults::memory_max,
                        description: t::settings::java::defaults::memory_max_desc,
                        widget: create_memory_max_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::java::defaults::enable_jvm_flags,
                        description: t::settings::java::defaults::enable_jvm_flags_desc,
                        widget: SettingItemWidget::Backend(Rc::new(|backend, _, cx| {
                            let enabled = backend.jvm_flags.as_ref().is_some_and(|flags| flags.enabled);
                            Switch::new("enable-default-jvm-flags").checked(enabled).on_click(cx.listener(|root, val, _, cx| {
                                let Some(backend) = root.backend_config().cloned() else {
                                    return;
                                };
                                let mut jvm_flags = backend.jvm_flags.unwrap_or_default();
                                if jvm_flags.enabled == *val {
                                    return;
                                }
                                jvm_flags.enabled = *val;
                                root.set_launch_defaults(
                                    backend.memory.clone(),
                                    Some(jvm_flags),
                                    backend.jvm_binary.clone(),
                                    cx,
                                );
                            })).into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::java::defaults::jvm_flags,
                        description: t::settings::java::defaults::jvm_flags_desc,
                        widget: create_jvm_flags_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::java::defaults::enable_jvm_binary,
                        description: t::settings::java::defaults::enable_jvm_binary_desc,
                        widget: SettingItemWidget::Backend(Rc::new(|backend, _, cx| {
                            let enabled = backend.jvm_binary.as_ref().is_some_and(|binary| binary.enabled);
                            Switch::new("enable-default-jvm-binary").checked(enabled).on_click(cx.listener(|root, val, _, cx| {
                                let Some(backend) = root.backend_config().cloned() else {
                                    return;
                                };
                                let mut jvm_binary = backend.jvm_binary.unwrap_or_default();
                                if jvm_binary.enabled == *val {
                                    return;
                                }
                                jvm_binary.enabled = *val;
                                root.set_launch_defaults(
                                    backend.memory.clone(),
                                    backend.jvm_flags.clone(),
                                    Some(jvm_binary),
                                    cx,
                                );
                            })).into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::java::defaults::jvm_binary,
                        description: t::settings::java::defaults::jvm_binary_desc,
                        widget: create_jvm_binary_widget(),
                        ..Default::default()
                    },
                ].into(),
                searched_items: None,
            },
        ].into(),
        searched_groups: None,
    }
}

fn create_memory_min_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let memory = backend.memory.clone().unwrap_or_default();
        let mut created = false;
        let state = window.use_keyed_state("default-memory-min", cx, |window, cx| {
            created = true;
            let mut state = InputState::new(window, cx);
            state.set_value(memory.min.to_string(), window, cx);
            state
        });
        if created {
            cx.subscribe_in(&state, window, |_, state, event: &NumberInputEvent, window, cx| {
                apply_memory_step(state, event, window, cx);
            }).detach();
            cx.subscribe(&state, |root, state, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let Some(backend) = root.backend_config().cloned() else {
                    return;
                };
                let min = state.read(cx).value().parse::<u32>().unwrap_or(0);
                let mut memory = backend.memory.unwrap_or_default();
                if memory.min == min {
                    return;
                }
                memory.min = min;
                root.set_launch_defaults(
                    Some(memory),
                    backend.jvm_flags.clone(),
                    backend.jvm_binary.clone(),
                    cx,
                );
            }).detach();
        } else {
            let state_read = state.read(cx);
            if state_read.value().parse::<u32>() != Ok(memory.min)
                && !state_read.focus_handle(cx).is_focused(window)
            {
                state.update(cx, |state, cx| {
                    state.set_value(memory.min.to_string(), window, cx);
                });
            }
        }
        NumberInput::new(&state).w(px(200.0)).disabled(!memory.enabled).into_any_element()
    }))
}

fn create_memory_max_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let memory = backend.memory.clone().unwrap_or_default();
        let mut created = false;
        let state = window.use_keyed_state("default-memory-max", cx, |window, cx| {
            created = true;
            let mut state = InputState::new(window, cx);
            state.set_value(memory.max.to_string(), window, cx);
            state
        });
        if created {
            cx.subscribe_in(&state, window, |_, state, event: &NumberInputEvent, window, cx| {
                apply_memory_step(state, event, window, cx);
            }).detach();
            cx.subscribe(&state, |root, state, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let Some(backend) = root.backend_config().cloned() else {
                    return;
                };
                let max = state.read(cx).value().parse::<u32>().unwrap_or(0);
                let mut memory = backend.memory.unwrap_or_default();
                if memory.max == max {
                    return;
                }
                memory.max = max;
                root.set_launch_defaults(
                    Some(memory),
                    backend.jvm_flags.clone(),
                    backend.jvm_binary.clone(),
                    cx,
                );
            }).detach();
        } else {
            let state_read = state.read(cx);
            if state_read.value().parse::<u32>() != Ok(memory.max)
                && !state_read.focus_handle(cx).is_focused(window)
            {
                state.update(cx, |state, cx| {
                    state.set_value(memory.max.to_string(), window, cx);
                });
            }
        }
        NumberInput::new(&state).w(px(200.0)).disabled(!memory.enabled).into_any_element()
    }))
}

fn apply_memory_step(
    state: &Entity<InputState>,
    event: &NumberInputEvent,
    window: &mut Window,
    cx: &mut App,
) {
    let NumberInputEvent::Step(step_action) = event;
    let Ok(mut value) = state.read(cx).value().parse::<u32>() else {
        return;
    };
    value = match step_action {
        StepAction::Decrement => value.saturating_div(256).saturating_sub(1).saturating_mul(256).max(128),
        StepAction::Increment => value.saturating_div(256).saturating_add(1).saturating_mul(256).max(128),
    };
    state.update(cx, |input, cx| {
        input.set_value(value.to_string(), window, cx);
    });
}

fn create_jvm_flags_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let jvm_flags = backend.jvm_flags.clone().unwrap_or_default();
        let mut created = false;
        let state = window.use_keyed_state("default-jvm-flags", cx, |window, cx| {
            created = true;
            let mut state = InputState::new(window, cx);
            state.set_value(jvm_flags.flags.to_string(), window, cx);
            state
        });
        if created {
            cx.subscribe(&state, |root, state, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let Some(backend) = root.backend_config().cloned() else {
                    return;
                };
                let value = state.read(cx).value();
                let mut jvm_flags = backend.jvm_flags.unwrap_or_default();
                if &*jvm_flags.flags == &*value {
                    return;
                }
                jvm_flags.flags = value.into();
                root.set_launch_defaults(
                    backend.memory.clone(),
                    Some(jvm_flags),
                    backend.jvm_binary.clone(),
                    cx,
                );
            }).detach();
        } else {
            let state_read = state.read(cx);
            if &*state_read.value() != &*jvm_flags.flags
                && !state_read.focus_handle(cx).is_focused(window)
            {
                state.update(cx, |state, cx| {
                    state.set_value(jvm_flags.flags.to_string(), window, cx);
                });
            }
        }
        Input::new(&state).w(px(280.0)).disabled(!jvm_flags.enabled).into_any_element()
    }))
}

fn create_jvm_binary_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, _, cx| {
        let jvm_binary = backend.jvm_binary.clone().unwrap_or_default();
        let path_label = jvm_binary.path.as_ref().map(|path| PathLabel::new(path.clone(), false));
        PathLabel::button_opt(&path_label, "select-default-jvm-binary")
            .disabled(!jvm_binary.enabled)
            .w(px(280.0))
            .min_w_0()
            .flex_shrink(1.0)
            .on_click(cx.listener(|root, _, window, cx| {
                root.select_file(t::settings::java::defaults::select_jvm_binary(), |root, path, cx| {
                    let Some(backend) = root.backend_config().cloned() else {
                        return;
                    };
                    let jvm_binary = InstanceJvmBinaryConfiguration {
                        enabled: backend.jvm_binary.as_ref().is_some_and(|binary| binary.enabled),
                        path,
                    };
                    root.set_launch_defaults(
                        backend.memory.clone(),
                        backend.jvm_flags.clone(),
                        Some(jvm_binary),
                        cx,
                    );
                }, window, cx);
            }))
            .into_any_element()
    }))
}
