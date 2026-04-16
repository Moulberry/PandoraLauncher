use std::sync::{Arc};

use bridge::{instance::InstanceID, message::{BridgeDataLoadState, MessageToBackend}, meta::MetadataRequest, serial::AtomicOptionSerial};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, WindowExt, button::{Button, ButtonVariants}, h_flex, notification::NotificationType, skeleton::Skeleton, tab::{Tab, TabBar}, text::TextView, v_flex
};
use rustc_hash::{FxHashMap};
use schema::{content::ContentSource, loader::Loader, modrinth::{
    ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectType,
}};

use crate::{
    component::error_alert::ErrorAlert, entity::{
        DataEntities,
        metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult},
    }, icon::PandoraIcon, pages::modrinth_page::{InstalledMod, PrimaryAction, env_display, format_downloads, get_primary_action, icon_for}
};

pub struct ModrinthProjectPage {
    data: DataEntities,
    project_id: SharedString,
    install_for: Option<InstanceID>,
    loading: Option<Subscription>,
    project: Option<Arc<ModrinthProjectResult>>,
    error: Option<SharedString>,
    active_tab: usize,
    can_install_latest: bool,
    installed_mods_by_project: FxHashMap<Arc<str>, Vec<InstalledMod>>,
    mods_load_state: Option<(BridgeDataLoadState, AtomicOptionSerial)>,
}

impl ModrinthProjectPage {
    pub fn new(
        project_id: SharedString,
        install_for: Option<InstanceID>,
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
                    let ContentSource::ModrinthProject { project_id } = &summary.content_source else {
                        continue;
                    };
                    let installed = installed_mods_by_project.entry(project_id.clone()).or_default();

                    let status = summary.update.status_if_matches(
                        instance.configuration.loader,
                        instance.configuration.minecraft_version.as_str().into()
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
                        let ContentSource::ModrinthProject { project_id } = &summary.content_source else {
                            continue;
                        };
                        let status = summary.update.status_if_matches(
                            instance.configuration.loader,
                            instance.configuration.minecraft_version.as_str().into()
                        );
                        let installed = page.installed_mods_by_project.entry(project_id.clone()).or_default();
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
        get_primary_action(
            project_id,
            self.can_install_latest,
            &self.installed_mods_by_project,
            cx,
        )
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

use crate::pages::page::Page;

impl Page for ModrinthProjectPage {
    fn controls(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }

    fn scrollable(&self, _cx: &App) -> bool {
        true
    }
}

impl Render for ModrinthProjectPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some((mods_state, load_serial)) = &self.mods_load_state
            && let Some(install_for) = self.install_for
        {
            mods_state.set_observed();
            if mods_state.should_load() {
                self.data.backend_handle.send_with_serial(MessageToBackend::RequestLoadMods { id: install_for }, load_serial);
            }
        }

        let content: AnyElement = if let Some(error) = &self.error {
            v_flex()
                .p_4()
                .child(ErrorAlert::new(
                    t::instance::content::error_loading().into(),
                    error.clone(),
                ))
                .into_any_element()
        } else if let Some(project) = &self.project {
            let project = Arc::clone(project);

            let icon = gpui::img(SharedUri::from(project.icon_url.as_ref().map(|url| url.to_string()).unwrap_or_else(|| "".to_string())))
                    .with_fallback(|| Skeleton::new().rounded_lg().size_20().into_any_element());

            let (env_icon, env_name) = env_display(project.client_side.unwrap(), project.server_side.unwrap());

            let categories_el: Option<AnyElement> = {
                let cats: Vec<_> = project.categories.iter()
                    .flat_map(|c| c.iter())
                    .chain(project.additional_categories.iter().flat_map(|c| c.iter()))
                    .collect();

                if cats.is_empty() {
                    None
                } else {
                    let text = cats
                        .iter()
                        .map(|c| t::modrinth::category::get(c.as_str(), false).unwrap_or(c.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    Some(h_flex()
                        .gap_1()
                        .text_sm()
                        .child(PandoraIcon::Tags)
                        .child(text)
                        .into_any_element())
                }
            };

            let stats = h_flex()
                .gap_4()
                .child(h_flex().gap_1()
                    .child(PandoraIcon::Download)
                    .child(format_downloads(project.downloads)))
                .when_some(categories_el, |this, el| this.child(el));

            let info_bar = h_flex()
                .gap_4()
                .items_center()
                .text_sm()
                .child(stats);

            let mut link_row = h_flex().gap_1().flex_wrap();

            let project_id_str = project.id.clone();
            let project_type = project.project_type;

            let install_button: AnyElement = {
                let data = self.data.clone();
                let install_for = self.install_for;
                let project_name = project.title.clone()
                    .map(SharedString::new)
                    .unwrap_or(t::instance::content::unnamed().into());

                let primary_action = if install_for.is_some() {
                    self.get_primary_action(&project_id_str, cx)
                } else {
                    PrimaryAction::Install
                };

                Button::new("install_project")
                    .label(primary_action.text())
                    .icon(primary_action.icon())
                    .with_variant(primary_action.button_variant())
                    .my_auto()
                    .px_6()
                    .on_click({
                        let project_name = project_name.clone();
                        let project_id_str = project_id_str.clone();
                        move |_, window, cx| {
                            if project_type != ModrinthProjectType::Other {
                                primary_action.perform(project_name.as_str(), &project_id_str, project_type, install_for, &data, window, cx);
                            } else {
                                window.push_notification(
                                    (NotificationType::Error, t::instance::content::install::unknown_type()),
                                    cx,
                                );
                            }
                        }
                    })
                    .into_any_element()
            };
            link_row = link_row.child(install_button);

            let slug = project.slug.as_deref()
                .unwrap_or(project.id.as_ref())
                .to_string();
            let project_type_str = project.project_type.as_str().to_string();
            link_row = link_row.child(
                Button::new("modrinth_web")
                    .label(t::modrinth::name())
                    .icon(PandoraIcon::ExternalLink)
                    .info()
                    .on_click({
                        let url = format!("https://modrinth.com/{}/{}", project_type_str, slug);
                        move |_, _, cx| { cx.open_url(&url); }
                    }),
            );

            if let Some(url) = &project.source_url {
                let url = url.clone();
                link_row = link_row.child(
                    Button::new("source")
                        .label(t::instance::content::links::source())
                        .icon(PandoraIcon::CodeXml)
                        .info()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.issues_url {
                let url = url.clone();
                link_row = link_row.child(
                    Button::new("issues")
                        .label(t::instance::content::links::issues())
                        .icon(PandoraIcon::Bug)
                        .info()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.wiki_url {
                let url = url.clone();
                link_row = link_row.child(
                    Button::new("wiki")
                        .label(t::instance::content::links::wiki())
                        .info()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.discord_url {
                let url = url.clone();
                link_row = link_row.child(
                    Button::new("discord").
                        label(t::instance::content::links::discord())
                        .info()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }

            let license_el: Option<AnyElement> = project.license.as_ref().map(|lic| {
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
                    .gap_1()
                    .text_sm()
                    .child(PandoraIcon::Scroll)
                    .child(display_id);

                if let Some(url) = url {
                    container = container
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            cx.open_url(&url);
                        });
                }

                container.into_any_element()
            });

            let versions_el: Option<AnyElement> = project.game_versions.as_deref()
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
                    h_flex()
                        .gap_1()
                        .text_sm()
                        .child(PandoraIcon::Layers)
                        .child(text)
                        .into_any_element()
                });

            let info_el: AnyElement = h_flex().gap_4()
                .when_some(versions_el, |this, el| this.child(el))
                .when_some(project.loaders.as_deref(), |this, loaders| {
                    this.children(loaders.iter().map(|loader| {
                        h_flex().gap_1()
                            .when_some(icon_for(loader.id()), |this, icon| {
                                this.child(Icon::empty().path(icon))
                            })
                            .child(loader.pretty_name())
                    }))
                })
                .child(h_flex().gap_1()
                    .child(env_icon)
                    .child(env_name))
                .when_some(license_el, |this, el| this.child(el))
                .into_any_element();

            let active_tab = self.active_tab;
            let tabs_el: AnyElement = TabBar::new("content_tabs").underline()
                .selected_index(active_tab)
                .on_click(cx.listener(|this, selected_index: &usize, _window, cx| {
                    this.active_tab = *selected_index;
                    cx.notify();
                }))
                .child(Tab::new().label(t::instance::content::tabs::description()))
                .child(Tab::new().label(t::instance::content::tabs::gallery()))
                .into_any_element();

            let body_el: AnyElement = match active_tab {
                0 => {
                    if let Some(body) = &project.body && !body.is_empty() {
                        v_flex()
                            .child(TextView::markdown("project_description", body.to_string()).gap_4())
                            .into_any_element()
                    } else {
                        v_flex()
                            .mt_2().pt_2()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(t::instance::content::no_description()))
                            .into_any_element()
                    }
                }
                1 => {
                    let gallery = project.gallery.as_deref().filter(|g| !g.is_empty());
                    v_flex()
                        .mt_2().pt_2()
                        .child(if let Some(images) = gallery {
                            h_flex()
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
                                                let url = img.url.clone();
                                                move |_, _, cx| { cx.open_url(&url); }
                                            }))
                                        .child(v_flex().p_1().max_w_full().min_w_0()
                                            .child(div().text_sm().child(SharedString::new(img.title.as_deref().unwrap_or_default())))
                                        )
                                })).into_any_element()
                        } else {
                            div().text_sm().text_color(cx.theme().muted_foreground).child(t::instance::content::no_gallery()).into_any_element()
                        })
                        .into_any_element()
                }
                _ => div().into_any_element(),
            };

            v_flex().p_4().gap_3().w_full()
                .child(h_flex().gap_4()
                    .child(icon.rounded_lg().size_24().min_w_24().min_h_24())
                    .child(v_flex().w_full().line_height(relative(1.1)).gap_2()
                        .child(div().text_xl().overflow_hidden().child(project.title.clone()
                            .map(SharedString::new)
                            .unwrap_or(t::instance::content::unnamed().into())))
                        .child(div().max_w(rems(40.0)).min_w_0().child(project.description.clone().map(SharedString::new).unwrap_or_default()))
                        .child(info_bar))
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

        content
    }
}
