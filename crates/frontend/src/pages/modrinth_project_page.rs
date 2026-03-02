use std::sync::{Arc, atomic::AtomicBool};

use bridge::{instance::{ContentUpdateStatus, InstanceContentID, InstanceID}, message::{AtomicBridgeDataLoadState, MessageToBackend}, meta::MetadataRequest, modal_action::ModalAction, serial::AtomicOptionSerial};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, StyledExt, WindowExt, button::{Button, ButtonVariants}, h_flex, notification::NotificationType, scroll::ScrollableElement, skeleton::Skeleton, tab::{Tab, TabBar}, text::TextView, v_flex
};
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{content::ContentSource, loader::Loader, modrinth::{
    ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectType,
    ModrinthSideRequirement,
}};

use crate::{
    component::{error_alert::ErrorAlert, page::Page, page_path::PagePath}, entity::{
        DataEntities,
        metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult},
    }, interface_config::InterfaceConfig, pages::modrinth_page::{InstalledMod, PrimaryAction, icon_for}, ts, ui
};

pub struct ModrinthProjectPage {
    data: DataEntities,
    project_id: SharedString,
    install_for: Option<InstanceID>,
    page_path: PagePath,
    loading: Option<Subscription>,
    project: Option<Arc<ModrinthProjectResult>>,
    error: Option<SharedString>,
    active_tab: usize,
    can_install_latest: bool,
    installed_mods_by_project: FxHashMap<Arc<str>, Vec<InstalledMod>>,
    mods_load_state: Option<(Arc<AtomicBridgeDataLoadState>, AtomicOptionSerial)>,
}

impl ModrinthProjectPage {
    pub fn new(
        project_id: SharedString,
        install_for: Option<InstanceID>,
        page_path: PagePath,
        data: &DataEntities,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut can_install_latest = false;
        let mut installed_mods_by_project: FxHashMap<Arc<str>, Vec<InstalledMod>> = FxHashMap::default();
        let mut mods_load_state = None;

        if let Some(install_for) = install_for {
            if let Some(entry) = data.instances.read(cx).entries.get(&install_for) {
                let instance = entry.read(cx);
                can_install_latest = instance.configuration.loader != Loader::Vanilla;

                let mods = instance.mods.read(cx);
                for summary in mods.iter() {
                    let ContentSource::ModrinthProject { project } = &summary.content_source else {
                        continue;
                    };
                    let installed = installed_mods_by_project.entry(project.clone()).or_default();
                    
                    let status = summary.update.status_if_matches(
                        instance.configuration.loader, 
                        instance.configuration.minecraft_version.as_str().into() // Konwersja na Ustr
                    );
                    
                    installed.push(InstalledMod {
                        mod_id: summary.id,
                        status,
                    });
                }

                mods_load_state = Some((instance.mods_state.clone(), AtomicOptionSerial::default()));

                let mods = instance.mods.clone();
                let instance_id = install_for;
                cx.observe(&mods, move |page, entity, cx| {
                    page.installed_mods_by_project.clear();
                    let instances = page.data.instances.read(cx);
                    let Some(instance_entry) = instances.entries.get(&instance_id) else { return };
                    let instance = instance_entry.read(cx);

                    let mods = entity.read(cx);
                    for summary in mods.iter() {
                        let ContentSource::ModrinthProject { project } = &summary.content_source else {
                            continue;
                        };
                        let status = summary.update.status_if_matches(
                            instance.configuration.loader, 
                            instance.configuration.minecraft_version.as_str().into()
                        );
                        let installed = page.installed_mods_by_project.entry(project.clone()).or_default();
                        installed.push(InstalledMod {
                            mod_id: summary.id,
                            status,
                        });
                    }
                }).detach();
            }
        }

        let mut page = Self {
            data: data.clone(),
            project_id,
            install_for,
            page_path,
            loading: None,
            project: None,
            error: None,
            active_tab: 0,
            can_install_latest,
            installed_mods_by_project,
            mods_load_state,
        };
        page.fetch_project(cx);
        page
    }

    fn get_primary_action(&self, project_id: &str, cx: &App) -> PrimaryAction {
        let install_latest = self.can_install_latest && !InterfaceConfig::get(cx).modrinth_install_normally;

        let installed = self.installed_mods_by_project.get(project_id);

        if let Some(installed) = installed && !installed.is_empty() {
            if !install_latest {
                return PrimaryAction::Reinstall;
            }

            let mut action = PrimaryAction::CheckForUpdates;
            for installed_mod in installed {
                match installed_mod.status {
                    ContentUpdateStatus::Unknown => {},
                    ContentUpdateStatus::AlreadyUpToDate => {
                        if !matches!(action, PrimaryAction::Update(..)) {
                            action = PrimaryAction::UpToDate;
                        }
                    },
                    ContentUpdateStatus::Modrinth => {
                        if let PrimaryAction::Update(vec) = &mut action {
                            vec.push(installed_mod.mod_id);
                        } else {
                            action = PrimaryAction::Update(vec![installed_mod.mod_id]);
                        }
                    },
                    _ => {
                        if action == PrimaryAction::CheckForUpdates {
                            action = PrimaryAction::ErrorCheckingForUpdates;
                        }
                    }
                };
            }
            return action;
        }

        if install_latest {
            PrimaryAction::InstallLatest
        } else {
            PrimaryAction::Install
        }
    }

    fn fetch_project(&mut self, cx: &mut Context<Self>) {
        let request = MetadataRequest::ModrinthProject(ModrinthProjectRequest {
            project_id: Arc::from(self.project_id.as_ref()),
        });

        let state = FrontendMetadata::request(&self.data.metadata, request, cx);

        let result: FrontendMetadataResult<ModrinthProjectResult> = state.read(cx).result();
        match result {
            FrontendMetadataResult::Loading => {
                let subscription = cx.observe(&state, |page, state, cx| {
                    let result: FrontendMetadataResult<ModrinthProjectResult> =
                        state.read(cx).result();
                    match result {
                        FrontendMetadataResult::Loading => {}
                        FrontendMetadataResult::Loaded(project) => {
                            page.project = Some(Arc::new(project.clone()));
                            page.loading = None;
                            cx.notify();
                        }
                        FrontendMetadataResult::Error(e) => {
                            page.error = Some(e);
                            page.loading = None;
                            cx.notify();
                        }
                    }
                });
                self.loading = Some(subscription);
            }
            FrontendMetadataResult::Loaded(project) => {
                self.project = Some(Arc::new(project.clone()));
            }
            FrontendMetadataResult::Error(e) => {
                self.error = Some(e);
            }
        }
    }
}

fn format_downloads(downloads: usize) -> SharedString {
    if downloads >= 1_000_000_000 {
        ts!("instance.content.downloads", num = format!("{}B", (downloads / 10_000_000) as f64 / 100.0))
    } else if downloads >= 1_000_000 {
        ts!("instance.content.downloads", num = format!("{}M", (downloads / 10_000) as f64 / 100.0))
    } else if downloads >= 10_000 {
        ts!("instance.content.downloads", num = format!("{}K", (downloads / 10) as f64 / 100.0))
    } else {
        ts!("instance.content.downloads", num = downloads)
    }
}

impl Render for ModrinthProjectPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let breadcrumb = self.page_path.create_breadcrumb(&self.data, cx);
        let theme = cx.theme().clone();

        if let Some((mods_state, load_serial)) = &self.mods_load_state
            && let Some(install_for) = self.install_for
        {
            let state = mods_state.load(std::sync::atomic::Ordering::SeqCst);
            if state.should_send_load_request() {
                self.data.backend_handle.send_with_serial(MessageToBackend::RequestLoadMods { id: install_for }, load_serial);
            }
        }

        let content: AnyElement = if let Some(error) = &self.error {
            v_flex()
                .p_4()
                .child(ErrorAlert::new(
                    "project_error",
                    "Error loading project".into(),
                    error.clone(),
                ))
                .into_any_element()
        } else if let Some(project) = &self.project {
            let project = Arc::clone(project);
            let install_for = self.install_for;

            let icon = gpui::img(SharedUri::from(project.icon_url.as_ref().map(|url| url.to_string()).unwrap_or_else(|| "".to_string())))
                    .with_fallback(|| Skeleton::new().rounded_lg().size_20().into_any_element());

            let (env_icon, env_name) = match (project.client_side.unwrap(), project.server_side.unwrap()) {
                (ModrinthSideRequirement::Required, ModrinthSideRequirement::Required) => {
                    (Icon::empty().path("icons/globe.svg"), ts!("modrinth.environment.client_and_server"))
                },
                (ModrinthSideRequirement::Required, ModrinthSideRequirement::Unsupported) => {
                    (Icon::empty().path("icons/computer.svg"), ts!("modrinth.environment.client_only"))
                },
                (ModrinthSideRequirement::Required, ModrinthSideRequirement::Optional) => {
                    (Icon::empty().path("icons/computer.svg"), ts!("modrinth.environment.client_only_server_optional"))
                },
                (ModrinthSideRequirement::Unsupported, ModrinthSideRequirement::Required) => {
                    (Icon::empty().path("icons/router.svg"), ts!("modrinth.environment.server_only"))
                },
                (ModrinthSideRequirement::Optional, ModrinthSideRequirement::Required) => {
                    (Icon::empty().path("icons/router.svg"), ts!("modrinth.environment.server_only_client_optional"))
                },
                (ModrinthSideRequirement::Optional, ModrinthSideRequirement::Optional) => {
                    (Icon::empty().path("icons/globe.svg"), ts!("modrinth.environment.client_or_server"))
                },
                _ => (Icon::empty().path("icons/cpu.svg"), ts!("modrinth.environment.unknown_environment")),
            };

            let gray = Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 };

            let stats = h_flex().gap_4()
                .child(h_flex().gap_1()
                    .child(Icon::empty().path("icons/download.svg"))
                    .child(format_downloads(project.downloads)))
                .child(h_flex().gap_1()
                    .child(env_icon)
                    .child(env_name));

            let loaders_row: AnyElement = project.loaders.as_deref()
                .filter(|l| !l.is_empty())
                .map(|loaders| {
                    h_flex().gap_4().pl_4().border_l_1().border_color(theme.border)
                        .children(loaders.iter().map(|loader| {
                            h_flex().gap_1()
                                .when_some(icon_for(loader.id()), |this, icon| {
                                    this.child(Icon::empty().path(icon))
                                })
                                .child(loader.pretty_name())
                        }))
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element());

            let info_bar = h_flex()
                .gap_4()
                .items_center()
                .text_sm()
                .child(stats)
                .child(loaders_row);

            let mut link_row = h_flex().gap_1().flex_wrap();

            let slug = project.slug.as_deref()
                .unwrap_or(project.id.as_ref())
                .to_string();
            let project_type_str = project.project_type.as_str().to_string();
            link_row = link_row.child(
                Button::new("modrinth_web")
                    .label("Modrinth").text_color(gray)
                    .icon(Icon::empty().path("icons/external-link.svg"))
                    .ghost()
                    .on_click({
                        let url = format!("https://modrinth.com/{}/{}", project_type_str, slug);
                        move |_, _, cx| { cx.open_url(&url); }
                    }),
            );

            if let Some(url) = &project.source_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("source").label("Source").text_color(gray).icon(Icon::empty().path("icons/code-xml.svg")).ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.issues_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("issues").label("Issues").text_color(gray).icon(Icon::empty().path("icons/bug.svg")).ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.wiki_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("wiki").label("Wiki").text_color(gray).ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.discord_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("discord").label("Discord").text_color(gray).ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }

            let license_el: AnyElement = if let Some(lic) = &project.license {
                let display_id = match lic.id.as_ref() {
                    "LicenseRef-All-Rights-Reserved" => "ARR".to_string(),
                    id if id.contains("LicenseRef") => id
                        .replace("LicenseRef-", "")
                        .replace("-", " "),
                    id => id.to_string(),
                };
                let url = lic.url.as_ref().map(|u| u.to_string());

                let mut container = h_flex()
                    .id("license")
                    .gap_2()
                    .text_sm()
                    .child(Icon::empty().path("icons/scroll.svg"))
                    .child(display_id);

                if let Some(url) = url {
                    container = container
                        .cursor_pointer() 
                        .on_click(move |_, _, cx| {
                            cx.open_url(&url);
                        });
                }

                container.into_any_element()
            } else {
                div().into_any_element()
            };

            let categories_el: AnyElement = {
                let cats: Vec<_> = project.categories.iter()
                    .flat_map(|c| c.iter())
                    .chain(project.additional_categories.iter().flat_map(|c| c.iter()))
                    .collect();

                if cats.is_empty() {
                    div().into_any_element()
                } else {
                    let text = cats.iter().map(|c| ts!(format!("modrinth.category.{}", c))).collect::<Vec<_>>().join(", ");
                    h_flex().gap_2().text_sm()
                        .child(Icon::empty().path("icons/tags.svg"))
                        .child(text)
                        .into_any_element()
                }
            };

            let versions_el: AnyElement = project.game_versions.as_deref()
                .filter(|v| !v.is_empty())
                .map(|gv| {
                    let text = if gv.len() <= 5 {
                        gv.iter().map(|v| v.as_ref()).collect::<Vec<_>>().join(", ")
                    } else {
                        format!("{} - {} ({} versions)",
                            gv.first().map(|v| v.as_ref()).unwrap_or(""),
                            gv.last().map(|v| v.as_ref()).unwrap_or(""),
                            gv.len())
                    };
                    h_flex().gap_2().text_sm()
                        .child(Icon::empty().path("icons/layers.svg"))
                        .child(text)
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element());

            let info_el: AnyElement = v_flex().child(license_el).child(categories_el).child(versions_el).into_any_element();

            let active_tab = self.active_tab;
            let tabs_el: AnyElement = TabBar::new("content_tabs").underline()
                .selected_index(active_tab)
                .on_click(cx.listener(|this, selected_index: &usize, _window, cx| {
                    this.active_tab = *selected_index;
                    cx.notify();
                }))
                .child(Tab::new().label("Description"))
                .child(Tab::new().label("Gallery"))
                .into_any_element();

            let body_el: AnyElement = match active_tab {
                0 => {
                    if let Some(body) = &project.body && !body.is_empty() {
                        v_flex()
                            .mt_2().pt_2()
                            .child(TextView::markdown("project_description", body.to_string()))
                            .into_any_element()
                    } else {
                        v_flex()
                            .mt_2().pt_2()
                            .child(div().text_sm().text_color(Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 }).child("No description available."))
                            .into_any_element()
                    }
                }
                1 => {
                    let gallery = project.gallery.as_deref().filter(|g| !g.is_empty());
                    v_flex()
                        .mt_2().pt_2()
                        .child(if let Some(images) = gallery {
                            h_flex()
                                .id("project_gallery")
                                .flex_wrap()
                                .gap_3()
                                .children(images.iter().enumerate().map(|(idx, img)| {
                                    v_flex().rounded_lg().h_80()
                                        .child(gpui::img(SharedUri::from(&img.url))
                                            .w_full()
                                            .h_72()
                                            .cursor_pointer()
                                            .rounded_t_lg()
                                            .id(("gallery_img", idx))
                                            .on_click({
                                                let url = img.url.to_string();
                                                move |_, _, cx| { cx.open_url(&url); }
                                            }))
                                        .child(v_flex().p_1().max_w_full().min_w_0()
                                            .child(div().text_sm().font_bold().child(img.title.as_deref().unwrap_or("").to_string()))
                                            //.child(div().text_xs().truncate().text_color(gray).child(img.description.as_deref().unwrap_or("").to_string()))
                                        )
                                })).into_any_element()
                        } else {
                            div().text_sm().text_color(Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 }).child("No gallery images.").into_any_element()
                        })
                        .into_any_element()
                }
                _ => div().into_any_element(),
            };

            let project_id_str = project.id.clone();
            let project_type = project.project_type;

            let install_button: AnyElement = {
                let data = self.data.clone();
                let install_for = self.install_for;
                let project_name: SharedString = project.title.as_deref().unwrap_or("Unnamed").to_string().into();

                let primary_action = if install_for.is_some() {
                    self.get_primary_action(&project_id_str, cx)
                } else {
                    PrimaryAction::Install
                };

                Button::new("install_project")
                    .label(primary_action.text())
                    .icon(primary_action.icon())
                    .with_variant(primary_action.button_variant())
                    .h_12()
                    .my_auto()
                    .px_6()
                    .on_click({
                        let project_name = project_name.clone();
                        let project_id_str = project_id_str.clone();
                        move |_, window, cx| {
                            if project_type != ModrinthProjectType::Other {
                                match &primary_action {
                                    PrimaryAction::Install | PrimaryAction::Reinstall => {
                                        crate::modals::modrinth_install::open(
                                            project_name.as_ref(),
                                            project_id_str.clone(),
                                            project_type,
                                            install_for,
                                            &data,
                                            window,
                                            cx,
                                        );
                                    },
                                    PrimaryAction::InstallLatest => {
                                        crate::modals::modrinth_install_auto::open(
                                            project_name.as_ref(),
                                            project_id_str.clone(),
                                            project_type,
                                            install_for.unwrap(),
                                            &data,
                                            window,
                                            cx,
                                        );
                                    },
                                    PrimaryAction::CheckForUpdates => {
                                        let modal_action = ModalAction::default();
                                        data.backend_handle.send(MessageToBackend::UpdateCheck {
                                            instance: install_for.unwrap(),
                                            modal_action: modal_action.clone(),
                                        });
                                        crate::modals::generic::show_notification(window, cx,
                                            ts!("instance.content.update.check.error"), modal_action);
                                    },
                                    PrimaryAction::ErrorCheckingForUpdates => {},
                                    PrimaryAction::UpToDate => {},
                                    PrimaryAction::Update(ids) => {
                                        for id in ids {
                                            let modal_action = ModalAction::default();
                                            data.backend_handle.send(MessageToBackend::UpdateContent {
                                                instance: install_for.unwrap(),
                                                content_id: *id,
                                                modal_action: modal_action.clone(),
                                            });
                                            crate::modals::generic::show_notification(window, cx,
                                                ts!("instance.content.update.error"), modal_action);
                                        }
                                    },
                                }
                            } else {
                                window.push_notification(
                                    (NotificationType::Error, ts!("instance.content.install.unknown_type")),
                                    cx,
                                );
                            }
                        }
                    })
                    .into_any_element()
            };
            
            v_flex().p_4().gap_3().w_full()
                .child(h_flex().gap_4()
                    .child(icon.rounded_lg().size_24().min_w_24().min_h_24())
                    .child(v_flex().w_full()
                        .child(h_flex().gap_4().mr_4().justify_between()
                            .child(v_flex().w_full().gap_2().mr_auto()
                                .child(div().h_6().text_xl().overflow_hidden().font_bold().child(project.title.as_deref().unwrap_or("Unnamed").to_string()))
                                .child(div().h_12().min_w_0().line_clamp(2).text_color(gray).text_xs().child(project.description.as_deref().unwrap_or("").to_string()))
                            )
                            .child(install_button)
                        )
                        .child(info_bar)
                    )
                )
                .child(link_row)
                .child(info_el)
                .child(tabs_el)
                .child(body_el)
                .into_any_element()
        } else {
            v_flex().p_4().gap_3().w_full()
                .child(h_flex().gap_4()
                    .child(Skeleton::new().rounded_lg().size_24().min_w_24().min_h_24())
                    .child(v_flex().w_full()
                        .child(h_flex().gap_4().mr_4().justify_between()
                            .child(v_flex().w_full().gap_2().mr_auto()
                                .child(Skeleton::new().h_6())
                                .child(Skeleton::new().h_12())
                            )
                            .child(Skeleton::new().h_12().w_32().rounded_md())
                        )
                        .child(Skeleton::new().h_6().w_full().rounded_md())
                    )
                )
                .child(Skeleton::new().h_6().w_64().rounded_md())
                .child(Skeleton::new().h_6().w_64().rounded_lg())
                .child(Skeleton::new().h_6().w_64().rounded_md())
                .into_any_element()
        };

        Page::new(breadcrumb).child(content).scrollable()
    }
}