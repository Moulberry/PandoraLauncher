use std::sync::Arc;

use bridge::{instance::InstanceID, meta::MetadataRequest};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    h_flex, label::Label,
    scroll::ScrollableElement,
    skeleton::Skeleton,
    v_flex,
};
use schema::modrinth::{
    ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectType,
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

fn render_markdown(body: &str, theme: &gpui_component::Theme) -> impl IntoElement {
    let mut elements: Vec<AnyElement> = Vec::new();

    let gray = Hsla { h: 0.0, s: 0.0, l: 0.55, a: 1.0 };
    let code_bg = Hsla { h: 0.0, s: 0.0, l: 0.12, a: 1.0 };

    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_lines: Vec<String> = Vec::new();
    let mut paragraph_lines: Vec<String> = Vec::new();

    let flush_paragraph = |lines: &mut Vec<String>, elements: &mut Vec<AnyElement>| {
        if lines.is_empty() { return; }
        let text = lines.join(" ");
        lines.clear();
        elements.push(
            div()
                .text_sm()
                .line_height(px(22.0))
                .mb_2()
                .child(text)
                .into_any_element()
        );
    };

    let flush_code = |lang: &mut String, lines: &mut Vec<String>, elements: &mut Vec<AnyElement>, code_bg: Hsla| {
        if lines.is_empty() { return; }
        let code_text = lines.join("\n");
        lines.clear();
        let label = if lang.is_empty() { "code".to_string() } else { lang.clone() };
        lang.clear();
        elements.push(
            v_flex()
                .mb_3()
                .rounded_md()
                .bg(code_bg)
                .p_3()
                .child(
                    div()
                        .text_xs()
                        .text_color(Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 })
                        .mb_1()
                        .child(label)
                )
                .child(
                    div()
                        .font_family("monospace")
                        .text_sm()
                        .child(code_text)
                )
                .into_any_element()
        );
    };

    for raw_line in body.lines() {
        if raw_line.starts_with("```") {
            if in_code_block {
                in_code_block = false;
                flush_code(&mut code_lang, &mut code_lines, &mut elements, code_bg);
            } else {
                flush_paragraph(&mut paragraph_lines, &mut elements);
                in_code_block = true;
                code_lang = raw_line.trim_start_matches('`').trim().to_string();
            }
            continue;
        }

        if in_code_block {
            code_lines.push(raw_line.to_string());
            continue;
        }

        if raw_line.trim().is_empty() {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            continue;
        }

        if raw_line.starts_with("#### ") {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            let text = raw_line.trim_start_matches('#').trim().to_string();
            elements.push(div().text_sm().font_bold().mt_3().mb_1().child(text).into_any_element());
            continue;
        }
        if raw_line.starts_with("### ") {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            let text = raw_line.trim_start_matches('#').trim().to_string();
            elements.push(div().text_base().font_bold().mt_3().mb_1().child(text).into_any_element());
            continue;
        }
        if raw_line.starts_with("## ") {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            let text = raw_line.trim_start_matches('#').trim().to_string();
            elements.push(div().text_lg().font_bold().mt_4().mb_1().child(text).into_any_element());
            continue;
        }
        if raw_line.starts_with("# ") {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            let text = raw_line.trim_start_matches('#').trim().to_string();
            elements.push(div().text_xl().font_bold().mt_4().mb_2().child(text).into_any_element());
            continue;
        }

        if raw_line.trim() == "---" || raw_line.trim() == "***" || raw_line.trim() == "___" {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            elements.push(
                div().h_px().w_full().bg(Hsla { h: 0.0, s: 0.0, l: 0.2, a: 1.0 }).my_3().into_any_element()
            );
            continue;
        }

        if raw_line.starts_with("- ") || raw_line.starts_with("* ") {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            let text = strip_inline_markdown(&raw_line[2..]);
            elements.push(
                h_flex().gap_2().mb_1().items_start()
                    .child(div().mt_1().text_color(gray).child("•"))
                    .child(div().text_sm().line_height(px(22.0)).child(text))
                    .into_any_element()
            );
            continue;
        }
        if let Some(rest) = parse_ordered_list(raw_line) {
            flush_paragraph(&mut paragraph_lines, &mut elements);
            elements.push(
                h_flex().gap_2().mb_1().items_start()
                    .child(div().text_color(gray).text_sm().child(rest.0))
                    .child(div().text_sm().line_height(px(22.0)).child(strip_inline_markdown(&rest.1)))
                    .into_any_element()
            );
            continue;
        }

        let trimmed = raw_line.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') {
            if trimmed.to_lowercase().starts_with("<summary>") && trimmed.to_lowercase().ends_with("</summary>") {
                let inner = &trimmed[9..trimmed.len()-10];
                elements.push(
                    div().text_sm().font_bold().mt_2().text_color(gray).child(inner.to_string()).into_any_element()
                );
            }
            continue;
        }

        paragraph_lines.push(strip_inline_markdown(raw_line));
    }

    flush_paragraph(&mut paragraph_lines, &mut elements);
    if in_code_block {
        flush_code(&mut code_lang, &mut code_lines, &mut elements, code_bg);
    }

    v_flex()
        .gap_0()
        .children(elements)
}

fn strip_inline_markdown(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '!' && i + 1 < chars.len() && chars[i+1] == '[' {
            if let Some(end) = find_closing_paren(&chars, i+1) {
                i = end + 1;
                continue;
            }
        }
        if chars[i] == '[' {
            if let Some(bracket_end) = chars[i+1..].iter().position(|&c| c == ']') {
                let text_start = i + 1;
                let text_end = i + 1 + bracket_end;
                let text: String = chars[text_start..text_end].iter().collect();
                if text_end + 1 < chars.len() && chars[text_end + 1] == '(' {
                    if let Some(paren_end) = chars[text_end+2..].iter().position(|&c| c == ')') {
                        out.push_str(&text);
                        i = text_end + 2 + paren_end + 1;
                        continue;
                    }
                }
            }
        }
        if i + 1 < chars.len() && ((chars[i] == '*' && chars[i+1] == '*') || (chars[i] == '_' && chars[i+1] == '_')) {
            i += 2;
            continue;
        }
        if chars[i] == '*' || chars[i] == '_' {
            i += 1;
            continue;
        }
        if chars[i] == '`' {
            i += 1;
            continue;
        }
        if i + 1 < chars.len() && chars[i] == '~' && chars[i+1] == '~' {
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    out
}

fn find_closing_paren(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, &c) in chars[start..].iter().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 { return Some(start + i); }
            }
            _ => {}
        }
    }
    None
}

fn parse_ordered_list(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let dot_pos = trimmed.find(". ")?;
    let prefix = &trimmed[..dot_pos];
    if prefix.chars().all(|c| c.is_ascii_digit()) && !prefix.is_empty() {
        Some((format!("{}.", prefix), trimmed[dot_pos+2..].to_string()))
    } else {
        None
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

            /*let gallery_el: AnyElement = project.gallery.as_deref()
                .filter(|g| !g.is_empty())
                .map(|images| {
                    div().border_2().border_color(green())
                        .id("project_gallery_scroll")
                        .h_48()
                        .child(
                            h_flex().border_2().border_color(red())
                                .id("project_gallery")
                                .overflow_x_scroll()
                                .h_full()
                                .gap_3()
                                .children(images.iter().enumerate().map(|(idx, img)| {
                                    gpui::img(SharedUri::from(&img.url))
                                        .size_full()
                                        .rounded_lg()
                                        .bg(theme.border)
                                        .object_fit(gpui::ObjectFit::Contain)
                                        .cursor_pointer()
                                        .id(("gallery_img", idx))
                                        .on_click({
                                            let url = img.url.to_string();
                                            move |_, _, cx| { cx.open_url(&url); }
                                        })
                                }))
                            )
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element());*/

            let body_el: AnyElement = if let Some(body) = &project.body && !body.is_empty() {
                v_flex()
                    .mt_2().pt_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(render_markdown(body, &theme))
                    .into_any_element()
            } else {
                div().into_any_element()
            };

            v_flex()
                .p_4()
                .gap_3()
                .id("modrinth_project_page")
                .overflow_y_scroll()
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
                .child(license_el)
                .child(loaders_el)
                .child(versions_el)
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

        ui::page(cx, breadcrumb).child(content)
    }
}