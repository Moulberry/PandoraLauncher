use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bridge::{
    instance::InstanceID,
    modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType},
};

use crate::{BackendState, copy_content_recursive, is_single_component_path_str};

pub async fn duplicate_instance(
    backend: Arc<BackendState>,
    id: InstanceID,
    name: &str,
    modal_action: ModalAction,
) {
    if !is_single_component_path_str(name) {
        modal_action.set_error_message(format!("Unable to duplicate instance, name must not be a path: {name}").into());
        modal_action.set_finished();
        return;
    }
    if !sanitize_filename::is_sanitized_with_options(name, sanitize_filename::OptionsForCheck { windows: true, ..Default::default() }) {
        modal_action.set_error_message(format!("Unable to duplicate instance, name is invalid: {name}").into());
        modal_action.set_finished();
        return;
    }
    if backend.instance_state.read().instances.iter().any(|i| i.name == name) {
        modal_action.set_error_message("Unable to duplicate instance, name is already used".to_string().into());
        modal_action.set_finished();
        return;
    }

    let source = {
        let mut state = backend.instance_state.write();
        let Some(instance) = state.instances.get_mut(id) else {
            modal_action.set_error_message("Unable to duplicate instance, unknown id".to_string().into());
            modal_action.set_finished();
            return;
        };
        instance.root_path.clone()
    };

    let dest = backend.directories.instances_dir.join(name);

    if let Err(err) = std::fs::create_dir(&dest) {
        modal_action.set_error_message(format!("Unable to create instance directory: {err}").into());
        modal_action.set_finished();
        return;
    }

    let tracker = ProgressTracker::new("Copying instance files...".into(), backend.send.clone());
    modal_action.trackers.push(tracker.clone());

    let cancel_title_shown = AtomicBool::new(false);
    let result = copy_content_recursive(&source, &dest, true, &|current, total| {
        tracker.set_count(current as usize);
        tracker.set_total(total as usize);
        tracker.notify();
    }, &|| {
        if modal_action.has_requested_cancel() {
            if !cancel_title_shown.swap(true, Ordering::Relaxed) {
                tracker.set_title("Cancelling...".into());
                tracker.notify();
            }
            Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Operation cancelled"))
        } else {
            Ok(())
        }
    });

    match result {
        Ok(()) => {
            tracker.set_finished(ProgressTrackerFinishType::Normal);
            tracker.notify();
        },
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dest);
            if modal_action.has_requested_cancel() {
                tracker.set_finished(ProgressTrackerFinishType::Fast);
                tracker.notify();
            } else {
                tracker.set_finished(ProgressTrackerFinishType::Error);
                tracker.notify();
                modal_action.set_error_message(error.to_string().into());
            }
        },
    }

    modal_action.set_finished();
}
