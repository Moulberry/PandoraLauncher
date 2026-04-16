use std::sync::Arc;

use bridge::{message::{AccountCapesResult, AccountSkinResult, MessageToBackend, UrlOrFile}, modal_action::ModalAction};
use futures::FutureExt;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, WindowExt, button::{Button, ButtonVariants}, h_flex, input::{Input, InputState}, notification::{Notification, NotificationType}, popover::Popover, scroll::ScrollableElement, skeleton::Skeleton, spinner::Spinner, v_flex}
;
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use schema::{minecraft_profile::{SkinState, SkinVariant}, unique_bytes::UniqueBytes};
use uuid::Uuid;
use crate::{
    component::{player_model_widget::PlayerModelWidget, shrinking_text::ShrinkingText}, data_asset_loader::DataAssetLoader, entity::{DataEntities, account::AccountExt}, icon::PandoraIcon, interface_config::InterfaceConfig, pages::page::Page, png_render_cache::ImageTransformation, skin_thumbnail_cache::SkinThumbnailCache, skin_renderer::determine_skin_variant,
};

pub struct SkinsPage {
    account_skins: FxHashMap<Uuid, AccountSkinResult>,
    account_capes: FxHashMap<Uuid, AccountCapesResult>,
    pending_login: Option<ModalAction>,
    applying_to_account: Option<Uuid>,
    request_account_skin: Option<Task<()>>,
    selected_skin: UniqueBytes,
    selected_cape: Option<(Uuid, Arc<str>)>,
    active_cape: Option<(Uuid, Arc<str>)>,
    pending_apply_cape: bool,
    player_model_widget: Entity<PlayerModelWidget>,
    skin_download_popover_open: bool,
    skin_download_input: Entity<InputState>,
    copy_skin_popover_open: bool,
    copy_skin_input: Entity<InputState>,
    add_from_file_task: Task<()>,
    data: DataEntities,
    skin_thumbnail_cache: Entity<SkinThumbnailCache>,
}

static DEFAULT_SKIN: Lazy<UniqueBytes> = Lazy::new(|| UniqueBytes::new(include_bytes!("../../../../assets/images/default_skin.png")));

impl SkinsPage {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut App) -> Self {
        Self {
            account_skins: FxHashMap::default(),
            account_capes: FxHashMap::default(),
            pending_login: None,
            applying_to_account: None,
            request_account_skin: None,
            selected_skin: DEFAULT_SKIN.clone(),
            selected_cape: None,
            active_cape: None,
            pending_apply_cape: false,
            player_model_widget: cx.new(|cx| PlayerModelWidget::new(cx, DEFAULT_SKIN.clone())),
            skin_download_popover_open: false,
            skin_download_input: cx.new(|cx| InputState::new(window, cx)),
            copy_skin_popover_open: false,
            copy_skin_input: cx.new(|cx| InputState::new(window, cx)),
            add_from_file_task: Task::ready(()),
            data: data.clone(),
            skin_thumbnail_cache: SkinThumbnailCache::new(cx),
        }
    }

    fn can_request_account_skin(&self) -> bool {
        if self.request_account_skin.is_some() {
            return false;
        }
        !self.has_pending_login()
    }

    fn select_skin(&mut self, skin: UniqueBytes, variant: SkinVariant, cx: &mut Context<Self>) {
        self.selected_skin = skin.clone();
        self.player_model_widget.update(cx, |widget, cx| {
            widget.set_skin(cx, skin, variant);
        });
    }

    fn select_cape(&mut self, id: Uuid, url: Arc<str>) {
        self.selected_cape = Some((id, url));
        self.pending_apply_cape = true;
    }

    fn select_no_cape(&mut self, cx: &mut Context<Self>) {
        self.selected_cape = None;
        self.pending_apply_cape = false;
        self.player_model_widget.update(cx, |widget, cx| {
            widget.set_cape(cx, None);
        });
    }

    fn request_account_skin(&mut self, uuid: Uuid, cx: &mut Context<Self>) {
        self.pending_login = None;

        let (send, recv) = tokio::sync::oneshot::channel();
        self.data.backend_handle.send(MessageToBackend::GetAccountSkin {
            account: uuid,
            result: send
        });
        let (send2, recv2) = tokio::sync::oneshot::channel();
        self.data.backend_handle.send(MessageToBackend::GetAccountCapes {
            account: uuid,
            result: send2
        });

        self.request_account_skin = Some(cx.spawn(async move |page, cx| {
            let skin_result = recv.await;
            let capes_result = recv2.await;

            let _ = page.update(cx, move |page, cx| {
                // Handle skin result
                let mut new_skin = None;
                if let Ok(skin_result) = skin_result {
                    if let AccountSkinResult::Success { skin, variant } = &skin_result {
                        if let Some(skin) = skin.clone() {
                            page.selected_skin = skin.clone();
                            new_skin = Some((skin, *variant));
                        }
                    }
                    page.account_skins.insert(uuid, skin_result);
                }

                // Handle cape result
                page.active_cape = None;
                page.selected_cape = None;
                page.pending_apply_cape = false;
                if let Ok(capes_result) = capes_result {
                    if let AccountCapesResult::Success { capes } = &capes_result {
                        for cape in capes {
                            if cape.state == SkinState::Active {
                                page.active_cape = Some((cape.id, cape.url.clone()));
                                page.selected_cape = page.active_cape.clone();
                                page.pending_apply_cape = true;
                                break;
                            }
                        }
                    }
                    page.account_capes.insert(uuid, capes_result);
                }

                // Update model widget
                page.player_model_widget.update(cx, |widget, cx| {
                    if let Some((skin, variant)) = new_skin {
                        widget.set_skin_and_cape(cx, skin, variant, None);
                    } else {
                        widget.set_cape(cx, None);
                    }
                });
                page.applying_to_account = None;
                page.request_account_skin = None;
                cx.notify();
            });
        }));
    }

    fn has_pending_login(&self) -> bool {
        if let Some(pending_login) = &self.pending_login {
            !pending_login.get_finished_at().is_some() && !pending_login.has_requested_cancel()
        } else {
            false
        }
    }
}

impl Page for SkinsPage {
    fn controls(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }

    fn scrollable(&self, _cx: &App) -> bool {
        false
    }
}

impl Render for SkinsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let secondary = theme.secondary;
        let secondary_hover = theme.secondary_hover;
        let radius = theme.radius;
        let list_active = theme.list_active;
        let list_active_border = theme.list_active_border;
        let secondary_skeleton = theme.secondary_foreground.opacity(0.5);

        if self.pending_apply_cape && let Some((_, cape_url)) = &self.selected_cape {
            let uri: SharedUri = SharedString::new(cape_url.clone()).into();
            let bytes = window.use_asset::<DataAssetLoader>(&Resource::Uri(uri), cx).flatten();
            if let Some(bytes) = bytes {
                self.player_model_widget.update(cx, |widget, cx| {
                    widget.set_cape(cx, Some(bytes));
                });
                self.pending_apply_cape = false;
            }
        }

        let mut library = v_flex()
            .w_full()
            .text_xl()
            .content_start()
            .items_start();

        let mut active_skin = None;
        let mut active_skin_variant = None;
        let controls;

        if let Some(account) = &self.data.accounts.read(cx).selected_account {
            let uuid = account.uuid;
            let username = account.username(InterfaceConfig::get(cx).hide_usernames);
            if account.offline {
                controls = t::skins::no_offline().into_any_element();
            } else if self.applying_to_account == Some(uuid) {
                if let Some(AccountSkinResult::Success { skin, variant }) = self.account_skins.get(&uuid) {
                    active_skin = skin.clone();
                    active_skin_variant = Some(*variant);
                }

                controls = h_flex()
                    .gap_2()
                    .child(Button::new("reset-changes")
                        .label(t::common::reset())
                        .disabled(true))
                    .child(Button::new("apply-changes")
                        .flex_1()
                        .label(t::common::apply_changes())
                        .success()
                        .icon(Spinner::new())
                        .loading(true))
                    .into_any_element();
            } else {
                match self.account_skins.get(&uuid) {
                    Some(AccountSkinResult::Success { skin, variant }) => {
                        active_skin = skin.clone();
                        active_skin_variant = Some(*variant);
                        let selected_variant = self.player_model_widget.read(cx).get_variant();
                        let can_apply_changes = if let Some(skin) = skin {
                            *skin != self.selected_skin
                                || *variant != selected_variant
                                || self.active_cape != self.selected_cape
                        } else {
                            self.active_cape != self.selected_cape
                        };
                        controls = h_flex()
                            .gap_2()
                            .child(Button::new("reset-changes")
                                .label(t::common::reset())
                                .disabled(!can_apply_changes)
                                .on_click({
                                    let skin = skin.clone();
                                    let variant = *variant;
                                    cx.listener(move |page, _, _, cx| {
                                        if let Some((cape_id, cape_url)) = &page.active_cape {
                                            page.select_cape(*cape_id, cape_url.clone());
                                        } else {
                                            page.select_no_cape(cx);
                                        }

                                        if let Some(skin) = skin.clone() {
                                            page.select_skin(skin, variant, cx);
                                        }
                                        cx.notify();
                                    })
                                }))
                            .child(Button::new("apply-changes")
                                .flex_1()
                                .label(t::common::apply_changes())
                                .success()
                                .disabled(!can_apply_changes)
                                .on_click({
                                    let skin = skin.clone();
                                    cx.listener(move |page, _, _, cx| {
                                        if let Some(skin) = &skin && skin != &page.selected_skin {
                                            page.data.backend_handle.send(MessageToBackend::SetAccountSkin {
                                                account: uuid,
                                                skin: page.selected_skin.clone(),
                                                variant: selected_variant
                                            });
                                        }
                                        if page.active_cape != page.selected_cape {
                                            page.data.backend_handle.send(MessageToBackend::SetAccountCape {
                                                account: uuid,
                                                cape: page.selected_cape.as_ref().map(|(id, _)| *id)
                                            });
                                        }
                                        page.applying_to_account = Some(uuid);
                                        page.request_account_skin(uuid, cx);
                                        cx.notify();
                                    })
                                }))
                            .into_any_element();
                    },
                    Some(AccountSkinResult::NeedsLogin) => {
                        controls = Button::new("login")
                            .label(t::skins::login_to_view_edit(&username))
                            .success()
                            .on_click(cx.listener(move |page, _, window, cx| {
                                let modal_action = ModalAction::default();
                                page.pending_login = Some(modal_action.clone());
                                page.account_skins.remove(&uuid);
                                page.account_capes.remove(&uuid);

                                page.data.backend_handle.send(MessageToBackend::Login {
                                    account: uuid,
                                    modal_action: modal_action.clone(),
                                });

                                let title: SharedString = t::login::title().into();
                                crate::modals::generic::show_modal(window, cx, title, t::login::error().into(), modal_action);
                            })
                        ).into_any_element();
                    },
                    Some(AccountSkinResult::UnableToLoadSkin) => {
                        controls = t::skins::unable_to_load(&username).into_any_element();
                    },
                    None => {
                        if self.can_request_account_skin() {
                            self.request_account_skin(uuid, cx);
                        }
                        controls = t::skins::loading(&username).into_any_element();
                    }
                }
            }

            if let Some(AccountCapesResult::Success { capes }) = self.account_capes.get(&uuid) {
                if !capes.is_empty() {
                    if InterfaceConfig::get(cx).collapse_capes_in_skins_page {
                        library = library
                            .child(h_flex()
                                .id("toggle-capes")
                                .child(t::skins::capes())
                                .child(PandoraIcon::ChevronLeft)
                                .on_click(|_, _, cx| {
                                    InterfaceConfig::get_mut(cx).collapse_capes_in_skins_page = false;
                                })
                            )
                    } else {
                        const TRANSFORM: ImageTransformation = ImageTransformation::CropAndScale {
                            min_x: 1,
                            min_y: 1,
                            width: 10,
                            height: 16,
                            scale: 5
                        };

                        let cape_buttons = capes.iter().enumerate().map(|(i, cape)| {
                            let selected = self.selected_cape.as_ref().map(|(id, _)| *id) == Some(cape.id);
                            let active = self.active_cape.as_ref().map(|(id, _)| *id) == Some(cape.id);
                            let padding = if selected {
                                px(7.0)
                            } else {
                                px(8.0)
                            };
                            let button = v_flex()
                                .gap_1()
                                .size(px(144.0))
                                .min_size(px(144.0))
                                .max_size(px(144.0))
                                .text_base()
                                .id(("select-cape", i))
                                .rounded(radius)
                                .items_center()
                                .justify_center()
                                .p(padding)
                                .when_else(selected, |this| {
                                    this.bg(list_active)
                                        .pt_0()
                                        .border_1()
                                        .border_color(list_active_border)
                                }, |this| {
                                    this.bg(secondary)
                                        .pt_px()
                                        .hover(|style| style.bg(secondary_hover))
                                })
                                .when(active, |this| {
                                    this.child(Icon::new(PandoraIcon::Flag).absolute().right(padding).bottom(padding))
                                })
                                .child(ShrinkingText::new(cape.alias.clone().into()))
                                .on_click({
                                    let cape_url = cape.url.clone();
                                    let uuid = cape.id;
                                    cx.listener(move |page, _, _, cx| {
                                        if page.selected_cape.as_ref().map(|(id, _)| *id) == Some(uuid) {
                                            page.select_no_cape(cx);
                                        } else {
                                            page.select_cape(uuid, cape_url.clone());
                                        }
                                        cx.notify();
                                    })
                                });

                            let uri: SharedUri = SharedString::new(cape.url.clone()).into();
                            let bytes = cx.fetch_asset::<DataAssetLoader>(&Resource::Uri(uri)).0.now_or_never().flatten();
                            if let Some(bytes) = bytes {
                                let cape_img = crate::png_render_cache::render_with_transform(bytes, TRANSFORM,  cx);
                                button.child(cape_img)
                            } else {
                                button.child(Skeleton::new().w(px(50.0)).h(px(80.0)).bg(secondary_skeleton))
                            }
                        });

                        library = library
                            .child(h_flex()
                                .id("toggle-capes")
                                .child(t::skins::capes())
                                .child(PandoraIcon::ChevronDown)
                                .on_click(|_, _, cx| {
                                    InterfaceConfig::get_mut(cx).collapse_capes_in_skins_page = true;
                                }))
                            .child(h_flex().w_full().mb_4().gap_2().flex_wrap().children(cape_buttons))
                    }
                }
            }
        } else {
            controls = "Select an account to view/edit skins".into_any_element();
        }

        let skin_library = self.data.use_skin_library(cx).cloned();
        let skin_library_iter = skin_library.iter().map(|l| l.skins.iter()).flatten();
        let skins = active_skin.iter().chain(skin_library_iter).enumerate();

        library = library
            .child(h_flex()
                .gap_3()
                .mb_1()
                .child(t::skins::title())
                .child(Button::new("add-file")
                    .label(t::skins::add_from_file())
                    .icon(PandoraIcon::File)
                    .success()
                    .small()
                    .compact()
                    .on_click({
                        cx.listener(move |page, _, window, cx| {
                            let receiver = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: true,
                                prompt: Some(t::skins::select_skin().into())
                            });

                            let entity = cx.entity();
                            let add_from_file_task = window.spawn(cx, async move |cx| {
                                let Ok(result) = receiver.await else {
                                    return;
                                };
                                _ = cx.update_window_entity(&entity, move |this, window, cx| {
                                    match result {
                                        Ok(Some(paths)) => {
                                            for path in paths {
                                                this.data.backend_handle.send(MessageToBackend::AddToSkinLibrary {
                                                    source: UrlOrFile::File { path }
                                                });
                                            }
                                        },
                                        Ok(None) => {},
                                        Err(error) => {
                                            let error = format!("{}", error);
                                            let notification = Notification::new()
                                                .autohide(false)
                                                .with_type(NotificationType::Error)
                                                .title(error);
                                            window.push_notification(notification, cx);
                                        },
                                    }
                                });
                            });
                            page.add_from_file_task = add_from_file_task;
                        })
                    }))
                .child(Popover::new("copy-skin-popover")
                    .trigger(Button::new("copy-skin").label(t::skins::copy_from_player()).icon(PandoraIcon::Download).success().small().compact())
                    .gap_2()
                    .w_full()
                    .items_start()
                    .child(Input::new(&self.copy_skin_input).w_128())
                    .open(self.copy_skin_popover_open)
                    .on_open_change({
                        let copy_skin_input = self.copy_skin_input.clone();
                        cx.listener(move |page, open, window, cx| {
                            if *open {
                                copy_skin_input.update(cx, |input, cx| {
                                    input.focus(window, cx);
                                });
                            }
                            page.copy_skin_popover_open = *open;
                        })
                    })
                    .child(Button::new("copy-skin-confirm")
                        .label(t::skins::copy())
                        .success()
                        .on_click({
                            cx.listener(move |page, _, _, cx| {
                                let value = page.copy_skin_input.read(cx).value();
                                let username: Arc<str> = value.into();
                                if !username.trim().is_empty() {
                                    page.data.backend_handle.send(MessageToBackend::CopyPlayerSkin {
                                        username,
                                    });
                                }
                                page.copy_skin_popover_open = false;
                                cx.notify();
                            })
                        })))
                .child(Popover::new("add-url-popover")
                    .trigger(Button::new("add-url").label(t::skins::add_from_url()).icon(PandoraIcon::Link).success().small().compact())
                    .gap_2()
                    .w_full()
                    .items_start()
                    .child(Input::new(&self.skin_download_input).w_128())
                    .open(self.skin_download_popover_open)
                    .on_open_change({
                        let skin_download_input = self.skin_download_input.clone();
                        cx.listener(move |page, open, window, cx| {
                            if *open {
                                skin_download_input.update(cx, |input, cx| {
                                    input.focus(window, cx);
                                });
                            }
                            page.skin_download_popover_open = *open;
                        })
                    })
                    .child(Button::new("download-skin")
                        .label(t::skins::download())
                        .success()
                        .on_click({
                            cx.listener(move |page, _, _, cx| {
                                let value = page.skin_download_input.read(cx).value();
                                let value: Arc<str> = value.into();
                                page.data.backend_handle.send(MessageToBackend::AddToSkinLibrary {
                                    source: UrlOrFile::Url { url: value }
                                });
                                page.skin_download_popover_open = false;
                                cx.notify();
                            })
                        })))
                .child(Button::new("open-folder")
                    .label(t::skins::open_folder())
                    .icon(PandoraIcon::FolderOpen)
                    .info()
                    .small()
                    .compact()
                    .when_some(skin_library.as_ref(), |this, skin_library| {
                        let folder = skin_library.folder.clone();
                        this.on_click(move |_, window, cx| {
                            crate::open_folder(&folder, window, cx);
                        })
                    }))
                .child(Button::new("toggle-3d")
                    .icon(if InterfaceConfig::get(cx).skin_list_show_3d { PandoraIcon::Image } else { PandoraIcon::Box })
                    .label(if InterfaceConfig::get(cx).skin_list_show_3d { t::skins::switch_view::texture() } else { t::skins::switch_view::model() })
                    .small()
                    .compact()
                    .on_click(cx.listener(|_, _, _, cx| {
                        InterfaceConfig::get_mut(cx).skin_list_show_3d ^= true;
                    }))))
            .child(h_flex().w_full().gap_2().flex_wrap().children(skins.filter_map(|(i, skin)| {
                let selected = &self.selected_skin == skin;
                let active = if let Some(active_skin) = &active_skin {
                    active_skin == skin
                } else {
                    false
                };
                if active && i > 0 {
                    return None;
                }

                let variant = if active && let Some(v) = active_skin_variant {
                    v
                } else {
                    crate::skin_renderer::determine_skin_variant(skin).unwrap_or(SkinVariant::Classic)
                };

                let show_3d = InterfaceConfig::get(cx).skin_list_show_3d;

                let padding = if selected {
                    px(7.0)
                } else {
                    px(8.0)
                };

                let skin_child: AnyElement = if show_3d {
                    let thumbnail = self.skin_thumbnail_cache.update(cx, |cache, cx| {
                        cache.get_or_queue(skin, variant, cx)
                    });
                    let thumb_w = px(crate::skin_thumbnail_cache::THUMB_WIDTH as f32);
                    let thumb_h = px(crate::skin_thumbnail_cache::THUMB_HEIGHT as f32);
                    if let Some(img) = thumbnail {
                        gpui::img(img)
                            .w(thumb_w)
                            .h(thumb_h)
                            .into_any_element()
                    } else {
                        Skeleton::new()
                            .w(thumb_w)
                            .h(thumb_h)
                            .bg(secondary_skeleton)
                            .into_any_element()
                    }
                } else {
                    crate::png_render_cache::render_with_transform(
                        skin.clone(),
                        ImageTransformation::ResizeToWidth { width: 128 },
                        cx,
                    ).into_any_element()
                };

                Some(div()
                    .id(("select-skin", i))
                    .rounded(radius)
                    .mb_4()
                    .text_base()
                    .p(padding)
                    .when_else(selected, |this| {
                        this.bg(list_active)
                            .border_1()
                            .border_color(list_active_border)
                    }, |this| {
                        this.bg(secondary)
                            .hover(|style| style.bg(secondary_hover))
                    })
                    .child(skin_child)
                    .when_else(active, |this| {
                        this.child(Icon::new(PandoraIcon::Flag).absolute().right(padding).bottom(padding))
                    }, |this| {
                        this.child(Button::new("delete-skin")
                            .icon(PandoraIcon::Trash2)
                            .occlude()
                            .danger()
                            .outline()
                            .compact()
                            .absolute()
                            .small()
                            .left(padding)
                            .bottom(padding)
                            .on_click({
                                let skin = skin.clone();
                                let active_skin = active_skin.clone();
                                cx.listener(move |page, _, window, cx| {
                                    page.data.backend_handle.send(MessageToBackend::RemoveFromSkinLibrary {
                                        skin: { skin.clone() }
                                    });
                                    cx.stop_propagation();
                                    window.prevent_default();
                                    if skin == page.selected_skin {
                                        if let Some(active_skin) = active_skin.clone() {
                                            let variant = if let Some(active_skin_variant) = active_skin_variant {
                                                active_skin_variant
                                            } else {
                                                determine_skin_variant(&active_skin).unwrap_or(SkinVariant::Classic)
                                            };
                                            page.select_skin(active_skin, variant, cx);
                                        }
                                    }
                                })
                            })
                        )
                    })
                    .on_click({
                        let skin = skin.clone();
                        cx.listener(move |page, _, _, cx| {
                            let variant = if active && let Some(active_skin_variant) = active_skin_variant {
                                active_skin_variant
                            } else {
                                crate::skin_renderer::determine_skin_variant(&skin).unwrap_or(SkinVariant::Classic)
                            };
                            page.select_skin(skin.clone(), variant, cx);
                        })
                    }))
            })));

        h_flex().p_4()
            .gap_4()
            .child(v_flex()
                    .gap_2()
                    .h_full()
                    .child(controls)
                    .child(self.player_model_widget.clone()))
            .child(library.overflow_y_scrollbar())
            .overflow_hidden()
    }
}
