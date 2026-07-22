use gpui::{prelude::*, *};
use gpui_component::{h_flex, v_flex};

use crate::{entity::instance::InstanceEntry, game_output::GameOutputRoot};

pub struct InstanceTerminalSubpage {
    output_root: Option<Entity<GameOutputRoot>>,
    output_entity_id: Option<EntityId>,
    _observe: Subscription,
}

impl InstanceTerminalSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = instance.read(cx).terminal_output.clone();
        let output_entity_id = current.as_ref().map(|entity| entity.entity_id());
        let output_root = current.map(|game_output| cx.new(|cx| GameOutputRoot::new_in_tab(game_output, window, cx)));

        // Rebuild the view if a new session is attached while this tab is open (relaunch).
        let _observe = cx.observe_in(instance, window, |this, instance, window, cx| {
            let new_output = instance.read(cx).terminal_output.clone();
            let new_id = new_output.as_ref().map(|entity| entity.entity_id());
            if new_id != this.output_entity_id {
                this.output_entity_id = new_id;
                this.output_root = new_output.map(|game_output| cx.new(|cx| GameOutputRoot::new_in_tab(game_output, window, cx)));
                cx.notify();
            }
        });

        Self {
            output_root,
            output_entity_id,
            _observe,
        }
    }
}

impl Render for InstanceTerminalSubpage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let content = if let Some(root) = self.output_root.clone() {
            root.into_any_element()
        } else {
            h_flex()
                .justify_center()
                .items_center()
                .size_full()
                .text_lg()
                .child(t::instance::terminal::no_output())
                .into_any_element()
        };

        v_flex().p_4().size_full().child(content)
    }
}
