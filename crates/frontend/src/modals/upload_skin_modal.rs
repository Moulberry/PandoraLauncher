use std::sync::Arc;

use bridge::{
    handle::BackendHandle,
    message::{MessageToBackend, UrlOrFile},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::Dialog,
    h_flex,
    input::{Input, InputState},
    v_flex,
};

use crate::{component::skin_renderer::SkinRenderer, data_asset_loader::DataAssetLoader};

pub struct UploadSkinModal {
    backend_handle: BackendHandle,
    custom_skin_url: Entity<InputState>,
}

impl UploadSkinModal {
    pub fn new(backend_handle: BackendHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let custom_skin_url =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://example.com/skin.png").clean_on_escape());
        Self {
            backend_handle,
            custom_skin_url,
        }
    }

    fn render(&mut self, modal: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        let url = self.custom_skin_url.read(cx).value();
        let url_trimmed = url.trim().to_string();
        let mut valid_skin = false;

        let preview = if url_trimmed.is_empty() {
            div()
                .size(px(168.0))
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Enter a skin URL")
                .into_any_element()
        } else {
            let uri: SharedUri = SharedString::new(url_trimmed.clone()).into();
            match window.use_asset::<DataAssetLoader>(&Resource::Uri(uri), cx).flatten() {
                Some(bytes) => {
                    if let Some(variant) = crate::skin_renderer::determine_skin_variant(&bytes) {
                        let renderer = SkinRenderer::new(
                            Some(Arc::from(bytes.to_vec().into_boxed_slice())),
                            variant == schema::minecraft_profile::SkinVariant::Slim,
                        );
                        if let Some(image) = renderer.render_to_buffer_with_params(168, 168, 0.3, 0.05, true) {
                            valid_skin = true;
                            canvas(
                                move |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let _ =
                                        window.paint_image(bounds, gpui::Corners::default(), image.clone(), 0, false);
                                },
                            )
                            .size(px(168.0))
                            .rounded_lg()
                            .bg(cx.theme().secondary)
                            .into_any_element()
                        } else {
                            invalid_preview("Invalid skin dimensions", cx)
                        }
                    } else {
                        invalid_preview("Invalid skin PNG", cx)
                    }
                },
                None => div()
                    .size(px(168.0))
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Loading preview...")
                    .into_any_element(),
            }
        };

        modal.title("Add skin from URL").child(
            v_flex()
                .gap_4()
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(preview)
                        .child(Input::new(&self.custom_skin_url).flex_1()),
                )
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("cancel").label(t::common::cancel()).on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        }))
                        .child(
                            Button::new("add-url")
                                .label(t::skins::add_from_url())
                                .success()
                                .disabled(!valid_skin)
                                .on_click({
                                    let backend_handle = self.backend_handle.clone();
                                    move |_, window, cx| {
                                        backend_handle.send(MessageToBackend::AddToSkinLibrary {
                                            source: UrlOrFile::Url {
                                                url: Arc::from(url_trimmed.as_str()),
                                            },
                                        });
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                ),
        )
    }
}

fn invalid_preview(message: &'static str, cx: &mut App) -> AnyElement {
    div()
        .size(px(168.0))
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().danger)
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().danger)
        .child(message)
        .into_any_element()
}

pub fn open(backend_handle: BackendHandle, window: &mut Window, cx: &mut App) {
    let state = cx.new(|cx| UploadSkinModal::new(backend_handle, window, cx));
    window.open_dialog(cx, move |modal, window, cx| {
        let modal = modal.w(px(560.0));
        state.update(cx, |state, cx| state.render(modal, window, cx))
    });
}
