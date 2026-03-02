use gpui_component::{Icon, IconNamed};
use gpui::*;

gpui_component::icon_named!(PandoraIcon, "../../assets/icons");

impl RenderOnce for PandoraIcon {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::new(self)
    }
}
