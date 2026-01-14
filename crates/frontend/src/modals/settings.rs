use std::{path::Path, sync::Arc};

use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, ThemeRegistry,
    button::{Button, ButtonVariants},
    select::{SearchableVec, Select, SelectEvent, SelectState},
    sheet::Sheet,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::interface_config::InterfaceConfig;

struct Settings {
    theme_folder: Arc<Path>,
    theme_select: Entity<SelectState<SearchableVec<SharedString>>>,
}

pub fn build_settings_sheet(
    theme_folder: Arc<Path>,
    window: &mut Window,
    cx: &mut App,
) -> impl Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static {
    let settings = cx.new(|cx| {
        let theme_select_delegate = SearchableVec::new(
            ThemeRegistry::global(cx)
                .sorted_themes()
                .iter()
                .map(|cfg| cfg.name.clone())
                .collect::<Vec<_>>(),
        );

        let theme_select = cx.new(|cx| {
            let mut state = SelectState::new(theme_select_delegate, Default::default(), window, cx).searchable(true);
            state.set_selected_value(&cx.theme().theme_name().clone(), window, cx);
            state
        });

        cx.subscribe_in(&theme_select, window, |_, entity, _: &SelectEvent<_>, _, cx| {
            let Some(theme_name) = entity.read(cx).selected_value().cloned() else {
                return;
            };

            InterfaceConfig::get_mut(cx).active_theme = theme_name.clone();

            let Some(theme) = gpui_component::ThemeRegistry::global(cx)
                .themes()
                .get(&SharedString::new(theme_name.trim_ascii()))
                .cloned()
            else {
                return;
            };

            gpui_component::Theme::global_mut(cx).apply_config(&theme);
        })
        .detach();

        Settings {
            theme_folder,
            theme_select,
        }
    });

    move |sheet, window, cx| {
        let tab_bar = TabBar::new("bar")
            .prefix(div().w_4())
            .selected_index(0)
            .underline()
            .child(Tab::new().label("Interface"))
            // .child(Tab::new().label("Game"))
            .on_click(|_index, _window, _cx| {
                // todo: switch
            });

        sheet.title("Settings").overlay_top(crate::root::sheet_margin_top(window)).p_0().child(
            v_flex()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(tab_bar)
                .child(settings.clone()),
        )
    }
}

impl Render for Settings {
    #[expect(unused_variables)]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .px_4()
            .py_3()
            .gap_3()
            .child(crate::labelled("Theme", Select::new(&self.theme_select)))
            .child(
                Button::new("open-theme-folder")
                    .info()
                    .icon(IconName::FolderOpen)
                    .label("Open theme folder")
                    .on_click({
                        let theme_folder = self.theme_folder.clone();
                        move |_, window, cx| {
                            crate::open_folder(&theme_folder, window, cx);
                        }
                    }),
            )
            .child(
                Button::new("open-theme-repo")
                    .info()
                    .icon(IconName::Globe)
                    .label("Open theme repository")
                    .on_click({
                        move |_, _, cx| {
                            cx.open_url("https://github.com/longbridge/gpui-component/tree/main/themes");
                        }
                    }),
            )
    }
}
