use std::sync::Arc;

use bridge::message::EmbeddedOrRaw;
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    v_flex,
};
use parking_lot::RwLock;

use crate::icon::PandoraIcon;

enum IconChoice {
    Embedded(&'static str),
    RawPng(&'static [u8]),
}

impl IconChoice {
    fn selected_icon(&self) -> EmbeddedOrRaw {
        match self {
            IconChoice::Embedded(icon) => EmbeddedOrRaw::Embedded((*icon).into()),
            IconChoice::RawPng(bytes) => EmbeddedOrRaw::Raw((*bytes).into()),
        }
    }
}

pub fn open_select_icon(selected: Box<dyn FnOnce(EmbeddedOrRaw, &mut App)>, window: &mut Window, cx: &mut App) {
    let select_file_task = Arc::new(RwLock::new(Task::ready(())));
    let selected = Arc::new(RwLock::new(Some(selected)));
    window.open_dialog(cx, move |dialog, _, _| {
        let mut icon_buttons = Vec::with_capacity(MINECRAFT_ICONS.len() + SVG_ICONS.len());

        icon_buttons.extend(MINECRAFT_ICONS.iter().enumerate().map(|(index, icon)| {
            let IconChoice::RawPng(bytes) = icon else {
                unreachable!();
            };

            Button::new(("minecraft", index))
                .success()
                .with_size(px(64.0))
                .child(
                    gpui::img(ImageSource::Resource(Resource::Embedded(MINECRAFT_ICON_PATHS[index].into()))).size_12(),
                )
                .on_click({
                    let selected = selected.clone();
                    move |_, window, cx| {
                        cx.stop_propagation();
                        if let Some(selected) = selected.write().take() {
                            (selected)(EmbeddedOrRaw::Raw((*bytes).into()), cx);
                        }
                        window.close_dialog(cx);
                    }
                })
                .into_any_element()
        }));

        icon_buttons.extend(SVG_ICONS.iter().enumerate().map(|(index, icon)| {
            Button::new(("svg", index))
                .success()
                .icon(Icon::default().path(*icon))
                .with_size(px(64.0))
                .on_click({
                    let selected = selected.clone();
                    move |_, window, cx| {
                        if let Some(selected) = selected.write().take() {
                            (selected)(IconChoice::Embedded(icon).selected_icon(), cx);
                        }
                        window.close_dialog(cx);
                    }
                })
                .into_any_element()
        }));

        let grid = div().grid().grid_cols(6).w_full().max_h_128().gap_2().children(icon_buttons);

        let content = v_flex()
            .size_full()
            .gap_2()
            .child(
                Button::new("custom")
                    .success()
                    .label(t::common::custom())
                    .icon(PandoraIcon::File)
                    .on_click({
                        let selected = selected.clone();
                        let select_file_task = select_file_task.clone();
                        move |_, window, cx| {
                            let receiver = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: false,
                                prompt: Some(t::instance::select_png_icon().into()),
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
                    }),
            )
            .child(grid);

        dialog.title(t::instance::select_icon()).child(content)
    });
}

static MINECRAFT_ICON_PATHS: &[&str] = &[
    "images/grass-block-icon.png",
    "images/obsidian-block-icon.png",
    "images/enchanted-golden-apple-icon.png",
    "images/diamond-sword-icon.png",
    "images/diamond-pickaxe-icon.png",
    "images/creeper-face-icon.png",
    "images/ender-pearl-icon.png",
    "images/nether-star-icon.png",
    "images/tnt-icon.png",
];

static MINECRAFT_ICONS: &[IconChoice] = &[
    IconChoice::RawPng(include_bytes!("../../../../assets/images/grass-block-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/obsidian-block-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/enchanted-golden-apple-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/diamond-sword-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/diamond-pickaxe-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/creeper-face-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/ender-pearl-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/nether-star-icon.png")),
    IconChoice::RawPng(include_bytes!("../../../../assets/images/tnt-icon.png")),
];

static SVG_ICONS: &[&str] = &[
    "icons/box.svg",
    "icons/swords.svg",
    "icons/camera.svg",
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
    "icons/compass.svg",
    "icons/cpu.svg",
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
