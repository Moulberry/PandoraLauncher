use schema::{
    backend_config::BackendConfig,
    instance::{
        InstanceConfiguration, InstanceJvmFlagsConfiguration,
    },
};

pub fn apply_global_launch_defaults(instance: &mut InstanceConfiguration, global: &BackendConfig) {
    let instance_memory_enabled = instance.memory.as_ref().is_some_and(|memory| memory.enabled);
    if !instance_memory_enabled {
        if let Some(memory) = &global.memory
            && memory.enabled
        {
            instance.memory = Some(*memory);
        }
    }

    let instance_binary_enabled = instance.jvm_binary.as_ref().is_some_and(|binary| binary.enabled);
    if !instance_binary_enabled {
        if let Some(jvm_binary) = &global.jvm_binary
            && jvm_binary.enabled
        {
            instance.jvm_binary = Some(jvm_binary.clone());
        }
    }

    let global_flags = global
        .jvm_flags
        .as_ref()
        .filter(|flags| flags.enabled && !flags.flags.trim_ascii().is_empty());
    let instance_flags = instance
        .jvm_flags
        .as_ref()
        .filter(|flags| flags.enabled && !flags.flags.trim_ascii().is_empty());

    match (global_flags, instance_flags) {
        (Some(global), Some(instance_flags)) => {
            instance.jvm_flags = Some(InstanceJvmFlagsConfiguration {
                enabled: true,
                flags: format!("{} {}", global.flags, instance_flags.flags).into(),
            });
        },
        (Some(global), None) => {
            instance.jvm_flags = Some(global.clone());
        },
        _ => {},
    }
}