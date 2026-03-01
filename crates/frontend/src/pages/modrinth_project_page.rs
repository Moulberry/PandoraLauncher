use std::sync::Arc;

use bridge::{instance::InstanceID, meta::MetadataRequest};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, StyledExt, button::{Button, ButtonVariants}, h_flex, scroll::ScrollableElement, skeleton::Skeleton, tab::{Tab, TabBar}, text::{TextView}, v_flex
};
use schema::modrinth::{
    ModrinthProjectRequest, ModrinthProjectResult,
    ModrinthSideRequirement,
};

use crate::{
    component::{error_alert::ErrorAlert, page_path::PagePath}, entity::{
        DataEntities,
        metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult},
    }, pages::modrinth_page::icon_for, ts, ui
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
        let mut page = Self {
            data: data.clone(),
            project_id,
            install_for,
            page_path,
            loading: None,
            project: None,
            error: None,
            active_tab: 0,
        };
        page.fetch_project(cx);
        page
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

            let categories_row: AnyElement = {
                let cats = project.categories.iter()
                    .flat_map(|c| c.iter())
                    .chain(project.additional_categories.iter().flat_map(|c| c.iter()));

                    h_flex().gap_4().pl_4().border_l_1().border_color(theme.border)
                        .children(cats.map(|category_id| {
                            h_flex().gap_1()
                                .when_some(icon_for(category_id), |this, icon| {
                                    this.child(Icon::empty().path(icon))
                                })
                                .child(ts!(format!("modrinth.category.{}", category_id.as_str())))
                        }))
                        .into_any_element()
            };

            let info_bar = h_flex()
                .gap_4()
                .items_center()
                .text_sm()
                .text_color(gray)
                .child(stats)
                .child(categories_row);

            let mut link_row = h_flex().gap_1().flex_wrap();

            let slug = project.slug.as_deref()
                .unwrap_or(project.id.as_ref())
                .to_string();
            let project_type_str = project.project_type.as_str().to_string();
            link_row = link_row.child(
                Button::new("modrinth_web")
                    .label("Modrinth")
                    .icon(IconName::ExternalLink)
                    .ghost()
                    .on_click({
                        let url = format!("https://modrinth.com/{}/{}", project_type_str, slug);
                        move |_, _, cx| { cx.open_url(&url); }
                    }),
            );

            if let Some(url) = &project.source_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("source").label("Source").icon(Icon::empty().path("icons/code-xml.svg")).ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.issues_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("issues").label("Issues").ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.wiki_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("wiki").label("Wiki").ghost()
                        .on_click(move |_, _, cx| { cx.open_url(&url); }),
                );
            }
            if let Some(url) = &project.discord_url {
                let url = url.to_string();
                link_row = link_row.child(
                    Button::new("discord").label("Discord").ghost()
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
                    .text_color(gray)
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

            let loaders_el: AnyElement = project.loaders.as_deref()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    let text = l.iter().map(|x| x.pretty_name()).collect::<Vec<_>>().join(", ");
                    h_flex().gap_2().text_sm().text_color(gray)
                        .child(Icon::empty().path("icons/puzzle.svg"))
                        .child(text)
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element());

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
                    h_flex().gap_2().text_sm().text_color(gray)
                        .child(Icon::empty().path("icons/layers.svg"))
                        .child(text)
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element());

            let info_el: AnyElement = v_flex().child(license_el).child(loaders_el).child(versions_el).into_any_element();

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

            v_flex()
                .p_4()
                .gap_3()
                .child(
                    h_flex().gap_4().items_start()
                        .child(icon.rounded_lg().size_20().min_w_20().min_h_20())
                        .child(
                            v_flex().gap_1().flex_1()
                                .child(div().text_2xl().font_bold()
                                    .child(project.title.as_deref().unwrap_or("Unnamed").to_string()))
                                .child(div().text_sm().text_color(gray)
                                    .child(project.description.as_deref().unwrap_or("").to_string()))
                                .child(info_bar)
                        )
                )
                .child(link_row)
                .child(info_el)
                .child(tabs_el)
                .child(body_el)
                .into_any_element()
        } else {
            v_flex().p_4().gap_4()
                .child(h_flex().gap_4()
                    .child(Skeleton::new().size_20().rounded_lg())
                    .child(v_flex().gap_2().flex_1()
                        .child(Skeleton::new().h_6().w(px(200.0)).rounded_md())
                        .child(Skeleton::new().h_4().w(px(300.0)).rounded_md())))
                .child(Skeleton::new().h_4().w(px(250.0)).rounded_md())
                .child(Skeleton::new().h(px(200.0)).w_full().rounded_lg())
                .child(Skeleton::new().h_4().w_full().rounded_md())
                .child(Skeleton::new().h_4().w_full().rounded_md())
                .into_any_element()
        };

        ui::page(cx, breadcrumb).child(content).overflow_y_scrollbar()
    }
}