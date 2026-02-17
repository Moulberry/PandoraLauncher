use std::sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc};

use bridge::{handle::BackendHandle, instance::InstanceID, message::EmbeddedOrRaw, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, h_flex, input::{Input, InputEvent, InputState}, scroll::ScrollableElement, v_flex, Disableable, Icon, IconName, Sizable, StyleSized, WindowExt
};
use parking_lot::RwLock;

pub fn open_select_icon(
    selected: Box<dyn FnOnce(EmbeddedOrRaw, &mut App)>,
    window: &mut Window,
    cx: &mut App,
) {
    let select_file_task = Arc::new(RwLock::new(Task::ready(())));
    let selected = Arc::new(RwLock::new(Some(selected)));
    window.open_dialog(cx, move |dialog, _, _| {
        let icons = ICONS.iter().enumerate().map(|(index, icon)| {
            let icon = *icon;
            Button::new(index).success().icon(Icon::default().path(icon)).with_size(px(64.0)).on_click({
                let selected = selected.clone();
                move |_, window, cx| {
                    if let Some(selected) = selected.write().take() {
                        (selected)(EmbeddedOrRaw::Embedded(icon.into()), cx);
                    }
                    window.close_dialog(cx);
                }
            })
        });

        let grid = div()
            .grid()
            .grid_cols(6)
            .w_full()
            .max_h_128()
            .gap_2()
            .children(icons);

        let content = v_flex()
            .size_full()
            .gap_2()
            .child(Button::new("custom").success().label("Custom").icon(IconName::File).on_click({
                let selected = selected.clone();
                let select_file_task = select_file_task.clone();
                move |_, window, cx| {
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some(SharedString::new_static("Select PNG Icon"))
                    });

                    let selected = selected.clone();
                    *select_file_task.write() = window.spawn(cx, async move |cx| {
                        let Ok(Ok(Some(result))) = receiver.await else {
                            return;
                        };
                        let Some(path) = result.first() else {
                            return;
                        };
                        let Ok(bytes) = std::fs::read(path) else {
                            return;
                        };
                        _ = cx.update(move |window, cx| {
                            if let Some(selected) = selected.write().take() {
                                (selected)(EmbeddedOrRaw::Raw(bytes.into()), cx);
                            }
                            window.close_dialog(cx);
                        });
                    });
                }
            }))
            .child(grid);

        dialog
            .title("Select Icon")
            .child(content)
    });

}

static ICONS: &[&'static str] = &[
    "icons/box.svg",
    "icons/swords.svg",
    "icons/camera.svg",
    "icons/brush.svg",
    "icons/house.svg",
    "icons/anvil.svg",
    "icons/archive.svg",
    "icons/asterisk.svg",
    "icons/award.svg",
    "icons/book.svg",
    "icons/bot.svg",
    "icons/briefcase.svg",
    "icons/bug.svg",
    "icons/building-2.svg",
    "icons/carrot.svg",
    "icons/cat.svg",
    "icons/compass.svg",
    "icons/cpu.svg",
    "icons/dollar-sign.svg",
    "icons/eye.svg",
    "icons/feather.svg",
    "icons/heart.svg",
    "icons/moon.svg",
    "icons/palette.svg",
    "icons/scroll.svg",
    "icons/square-terminal.svg",
    "icons/tree-pine.svg",
    "icons/wand-sparkles.svg",
    "icons/zap.svg",
];
