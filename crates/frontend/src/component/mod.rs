pub mod content_list;
pub mod error_alert;
pub mod horizontal_sections;
pub mod instance_dropdown;
pub mod instance_list;
pub mod menu;
pub mod mod_action_dropdown;
pub mod named_dropdown;
pub mod page_path;
pub mod path_label;
pub mod player_model;
pub mod player_model_widget;
pub mod progress_bar;
pub mod readonly_text_field;
pub mod resize_panel;
pub mod responsive_grid;
pub mod search_helper;
pub mod shrinking_text;
pub mod skin_renderer;
pub mod title_bar;

pub fn create_styled_text(text: &schema::text_component::FlatTextComponent, grayscale: bool) -> gpui::StyledText {
    gpui::StyledText::new(&text.content).with_highlights(text.runs.iter().map(|run| {
        (
            run.range.clone(),
            gpui::HighlightStyle {
                color: run.style.colour.map(|rgb| {
                    let hsla: gpui::Hsla = gpui::rgb(rgb).into();
                    if grayscale { hsla.grayscale() } else { hsla }
                }),
                font_weight: run.style.bold.map(|bold| {
                    if bold {
                        gpui::FontWeight::BOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    }
                }),
                font_style: run.style.italic.map(|italic| {
                    if italic {
                        gpui::FontStyle::Normal
                    } else {
                        gpui::FontStyle::Italic
                    }
                }),
                background_color: None,
                underline: run.style.underlined.map(|underline| {
                    if underline {
                        gpui::UnderlineStyle::default()
                    } else {
                        gpui::UnderlineStyle {
                            thickness: gpui::px(1.0),
                            ..Default::default()
                        }
                    }
                }),
                strikethrough: run.style.strikethrough.map(|strikethrough| {
                    if strikethrough {
                        gpui::StrikethroughStyle::default()
                    } else {
                        gpui::StrikethroughStyle {
                            thickness: gpui::px(1.0),
                            ..Default::default()
                        }
                    }
                }),
                fade_out: None,
            },
        )
    }))
}
