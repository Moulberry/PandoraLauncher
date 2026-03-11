use gpui_component::{Icon, IconNamed};
use gpui::*;

gpui_component::icon_named!(PandoraIcon, "../../assets/icons");

impl RenderOnce for PandoraIcon {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::new(self)
    }
}

impl PandoraIcon {
    pub fn pause_play(pause: bool) -> Self {
        if pause {
            Self::Pause
        } else {
            Self::Play
        }
    }
}
