use std::rc::Rc;

use gpui::{InteractiveElement, prelude::*, *};
use gpui_component::{button::Button, h_flex, popover::Popover};

use crate::icon::PandoraIcon;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModAction {
    Reinstall,
    OpenPage,
    UpdateCheck,
    Remove,
}

impl ModAction {
    pub fn text(&self) -> SharedString {
        match self {
            ModAction::Reinstall => "Reinstall".into(),
            ModAction::OpenPage => "Open page".into(),
            ModAction::UpdateCheck => "Update check".into(),
            ModAction::Remove => "Remove".into(),
        }
    }

    pub fn icon(&self) -> PandoraIcon {
        match self {
            ModAction::Reinstall => PandoraIcon::Download,
            ModAction::OpenPage => PandoraIcon::ExternalLink,
            ModAction::UpdateCheck => PandoraIcon::RefreshCcw,
            ModAction::Remove => PandoraIcon::Trash2,
        }
    }
}

pub fn render_mod_action_dropdown(
    id: SharedString,
    actions: Vec<ModAction>,
    on_action: Rc<dyn Fn(ModAction, &mut Window, &mut App)>,
    is_open: bool,
    on_toggle: Rc<dyn Fn(&mut Window, &mut App)>,
    accent: gpui::Hsla,
) -> impl IntoElement {
    Popover::new(id)
        .trigger(
            Button::new("installed-button")
                .label("Installed")
                .icon(PandoraIcon::ChevronDown)
                .h_10()
                .on_click({
                    let on_toggle = on_toggle.clone();
                    move |_, window, cx| {
                        on_toggle(window, cx);
                    }
                }),
        )
        .gap_2()
        .w_48()
        .items_start()
        .open(is_open)
        .children(actions.iter().map(move |action| {
            let on_action = on_action.clone();
            let action = *action;
            div()
                .id(action.text())
                .w_full()
                .px_3()
                .py_2()
                .text_sm()
                .rounded_md()
                .cursor_pointer()
                .hover(|style| style.bg(accent))
                .child(h_flex().gap_2().child(action.icon()).child(action.text()))
                .on_click(move |_, window, cx| {
                    on_action(action, window, cx);
                })
        }))
}
