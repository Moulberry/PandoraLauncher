use std::sync::Arc;

use bridge::{message::{AccountCapesResult, AccountSkinResult, MessageToBackend, UrlOrFile}, modal_action::ModalAction};
use futures::FutureExt;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, WindowExt, button::{Button, ButtonVariants}, h_flex, input::{Input, InputState}, notification::{Notification, NotificationType}, popover::Popover, scroll::ScrollableElement, skeleton::Skeleton, spinner::Spinner, v_flex}
;
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use schema::minecraft_profile::{SkinState, SkinVariant};
use uuid::Uuid;
use crate::{
    component::player_model_widget::PlayerModelWidget, data_asset_loader::DataAssetLoader, entity::DataEntities, icon::PandoraIcon, pages::page::Page, png_render_cache::ImageTransformation, ts
};

pub struct SkinsPage {
    account_skins: FxHashMap<Uuid, AccountSkinResult>,
    account_capes: FxHashMap<Uuid, AccountCapesResult>,
    pending_login: Option<ModalAction>,
    applying_to_account: Option<Uuid>,
    request_account_skin: Option<Task<()>>,
    selected_skin: Arc<[u8]>,
    selected_cape: Option<(Uuid, Arc<str>)>,
    active_cape: Option<(Uuid, Arc<str>)>,
    pending_apply_cape: bool,
    player_model_widget: Entity<PlayerModelWidget>,
    skin_download_popover_open: bool,
    skin_download_input: Entity<InputState>,
    add_from_file_task: Task<()>,
    data: DataEntities,
}

static DEFAULT_SKIN: Lazy<Arc<[u8]>> = Lazy::new(|| Arc::from(*include_bytes!("../../../../assets/images/default_skin.png")));

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
            add_from_file_task: Task::ready(()),
            data: data.clone(),
        }
    }

    fn can_request_account_skin(&self) -> bool {
        if self.request_account_skin.is_some() {
            return false;
        }
        !self.has_pending_login()
    }

    fn select_skin(&mut self, skin: Arc<[u8]>, cx: &mut Context<Self>) {
        self.selected_skin = skin.clone();
        self.player_model_widget.update(cx, |widget, cx| {
            widget.set_skin(cx, skin);
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
                if let Ok(skin_result) = skin_result {
                    if let AccountSkinResult::Success { skin } = &skin_result {
                        if let Some(skin) = skin.clone() {
                            page.selected_skin = skin;
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
                    widget.set_skin_and_cape(cx, page.selected_skin.clone(), None);
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
        let controls;

        if let Some(account) = &self.data.accounts.read(cx).selected_account {
            let uuid = account.uuid;
            let username = account.username.clone();
            if account.offline {
                controls = ts!("skins.no_offline").into_any_element();
            } else if self.applying_to_account == Some(uuid) {
                if let Some(AccountSkinResult::Success { skin }) = self.account_skins.get(&uuid) {
                    active_skin = skin.clone();
                }

                controls = h_flex()
                    .gap_2()
                    .child(Button::new("reset-changes")
                        .label(ts!("common.reset"))
                        .disabled(true))
                    .child(Button::new("apply-changes")
                        .flex_1()
                        .label(ts!("common.apply_changes"))
                        .success()
                        .icon(Spinner::new())
                        .loading(true))
                    .into_any_element();
            } else {
                match self.account_skins.get(&uuid) {
                    Some(AccountSkinResult::Success { skin }) => {
                        active_skin = skin.clone();
                        let can_apply_changes = if let Some(skin) = skin {
                            !Arc::ptr_eq(skin, &self.selected_skin) ||
                                self.active_cape != self.selected_cape
                        } else {
                            self.active_cape != self.selected_cape
                        };
                        controls = h_flex()
                            .gap_2()
                            .child(Button::new("reset-changes")
                                .label(ts!("common.reset"))
                                .disabled(!can_apply_changes)
                                .on_click({
                                    let skin = skin.clone();
                                    cx.listener(move |page, _, _, cx| {
                                        if let Some((cape_id, cape_url)) = &page.active_cape {
                                            page.select_cape(*cape_id, cape_url.clone());
                                        } else {
                                            page.select_no_cape(cx);
                                        }

                                        if let Some(skin) = skin.clone() {
                                            page.select_skin(skin, cx);
                                        }
                                        cx.notify();
                                    })
                                }))
                            .child(Button::new("apply-changes")
                                .flex_1()
                                .label(ts!("common.apply_changes"))
                                .success()
                                .disabled(!can_apply_changes)
                                .on_click({
                                    let skin = skin.clone();
                                    cx.listener(move |page, _, _, cx| {
                                        if let Some(skin) = &skin && !Arc::ptr_eq(&skin, &page.selected_skin) {
                                            let is_slim = crate::skin_renderer::is_slim(&page.selected_skin).unwrap_or(false);
                                            let variant = if is_slim {
                                                SkinVariant::Slim
                                            } else {
                                                SkinVariant::Classic
                                            };
                                            page.data.backend_handle.send(MessageToBackend::SetAccountSkin {
                                                account: uuid,
                                                skin: page.selected_skin.clone(),
                                                variant
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
                            .label(ts!("skins.login_to_view_edit", username = username))
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

                                let title: SharedString = ts!("login.title");
                                crate::modals::generic::show_modal(window, cx, title, ts!("login.error"), modal_action);
                            })
                        ).into_any_element();
                    },
                    Some(AccountSkinResult::UnableToLoadSkin) => {
                        controls = ts!("skins.unable_to_load", username = username).into_any_element();
                    },
                    None => {
                        if self.can_request_account_skin() {
                            self.request_account_skin(uuid, cx);
                        }
                        controls = ts!("skins.loading", username = username).into_any_element();
                    }
                }
            }

            if let Some(AccountCapesResult::Success { capes }) = self.account_capes.get(&uuid) {
                if !capes.is_empty() {
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
                            .min_size(px(144.0))
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
                            .child(SharedString::new(cape.alias.clone()))
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
                        .child(ts!("skins.capes"))
                        .child(h_flex().w_full().mb_4().gap_2().flex_wrap().children(cape_buttons))
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
                .child(ts!("skins.title"))
                .child(Button::new("add-file")
                    .label("Add from file")
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
                                prompt: Some("Select Skin".into())
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
                .child(Popover::new("add-url-popover")
                    .trigger(Button::new("add-url").label("Add from url").icon(PandoraIcon::Link).success().small().compact())
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
                        .label("Download")
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
                    .label("Open folder")
                    .icon(PandoraIcon::FolderOpen)
                    .info()
                    .small()
                    .compact()
                    .when_some(skin_library.as_ref(), |this, skin_library| {
                        let folder = skin_library.folder.clone();
                        this.on_click(move |_, window, cx| {
                            crate::open_folder(&folder, window, cx);
                        })
                    })))
            .child(h_flex().w_full().gap_2().flex_wrap().children(skins.filter_map(|(i, skin)| {
                let skin_img = crate::png_render_cache::render_with_transform(skin.clone(),
                    ImageTransformation::ResizeToWidth { width: 128 },  cx);
                let selected = Arc::ptr_eq(&self.selected_skin, skin);
                let active = if let Some(active_skin) = &active_skin {
                    Arc::ptr_eq(active_skin, skin)
                } else {
                    false
                };
                if active && i > 0 {
                    return None;
                }
                let padding = if selected {
                    px(7.0)
                } else {
                    px(8.0)
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
                    .when(active, |this| {
                        this.child(Icon::new(PandoraIcon::Flag).absolute().right(padding).bottom(padding))
                    })
                    .child(skin_img)
                    .on_click({
                        let skin = skin.clone();
                        cx.listener(move |page, _, _, cx| {
                            page.select_skin(skin.clone(), cx);
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
