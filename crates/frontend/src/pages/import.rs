use std::sync::Arc;

use bridge::{handle::BackendHandle, import::{ImportFromOtherLaunchers, OtherLauncher}, install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget}, message::MessageToBackend, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, checkbox::Checkbox, scroll::ScrollableElement, spinner::Spinner, v_flex, ActiveTheme as _, Disableable, Sizable
};
use schema::{content::ContentSource, loader::Loader};
use strum::IntoEnumIterator;

use crate::{component::{responsive_grid::ResponsiveGrid}, entity::DataEntities, pages::page::Page, root};

pub struct ImportPage {
    backend_handle: BackendHandle,
    import_from_other_launchers: Option<ImportFromOtherLaunchers>,
    import_from: Option<OtherLauncher>,
    import_accounts: bool,
    import_instances: bool,
    _get_import_paths_task: Task<()>,
    _open_file_task: Task<()>,
}

impl ImportPage {
    pub fn new(data: &DataEntities, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            backend_handle: data.backend_handle.clone(),
            import_from_other_launchers: None,
            import_from: None,
            import_accounts: true,
            import_instances: true,
            _get_import_paths_task: Task::ready(()),
            _open_file_task: Task::ready(()),
        };

        page.update_launcher_paths(cx);

        page
    }

    pub fn update_launcher_paths(&mut self, cx: &mut Context<Self>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        self._get_import_paths_task = cx.spawn(async move |page, cx| {
            let result: ImportFromOtherLaunchers = recv.await.unwrap_or_default();
            let _ = page.update(cx, move |page, cx| {
                page.import_from_other_launchers = Some(result);
                cx.notify();
            });
        });

        self.backend_handle.send(MessageToBackend::GetImportFromOtherLauncherPaths {
            channel: send,
        });
    }
}

impl Page for ImportPage {
    fn controls(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }

    fn scrollable(&self, _cx: &App) -> bool {
        true
    }
}

impl Render for ImportPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(imports) = &self.import_from_other_launchers else {
            let content = v_flex().size_full().p_3().gap_3()
                .child(Spinner::new().with_size(gpui_component::Size::Large));

            return content;
        };

        let mut content = v_flex().size_full().p_3().gap_3()
            .child(ResponsiveGrid::new(Size::new(AvailableSpace::MinContent, AvailableSpace::MinContent))
                .gap_2()
                .children({
                    OtherLauncher::iter().map(|launcher| {
                        Button::new(launcher.to_string())
                             .label(format!("Import from {}", launcher))
                             .w_full()
                              .disabled(imports.imports[launcher].is_none())
                             .on_click(cx.listener(move |page, _, _, _| page.import_from = Some(launcher)))
                     })
                })
                .child(Button::new("mrpack")
                    .label("Import Modrinth Pack (.mrpack)")
                    .w_full()
                    .on_click(cx.listener(|page, _, window, cx| {
                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                            files: true,
                            directories: false,
                            multiple: false,
                            prompt: Some("Select Modrinth Pack".into())
                        });
                        let page_entity = cx.entity();
                        page._open_file_task = window.spawn(cx, async move |cx| {
                            let Ok(Ok(Some(result))) = receiver.await else {
                                return;
                            };
                            let Some(path) = result.first() else {
                                return;
                            };
                            _ = page_entity.update_in(cx, |page, window, cx| {
                                let content_install = ContentInstall {
                                    target: InstallTarget::NewInstance { name: None },
                                    loader_hint: Loader::Unknown,
                                    version_hint: None,
                                    files: Arc::from([
                                        ContentInstallFile {
                                            replace_old: None,
                                            path: ContentInstallPath::Automatic,
                                            download: ContentDownload::File { path: path.into() },
                                            content_source: ContentSource::Manual,
                                        }
                                    ]),
                                };
                                root::start_install(content_install, &page.backend_handle, window, cx);
                            });
                        })
                    })))
            );

        if let Some(import_from) = self.import_from && let Some(import) = &imports.imports[import_from] {
            let label = format!("Import From {}", import_from);
            let import_accounts = self.import_accounts && import.can_import_accounts;
            content = content.child(v_flex()
                .w_full()
                .border_1()
                .gap_2()
                .p_2()
                .rounded(cx.theme().radius_lg)
                .border_color(cx.theme().border)
                .when(import.can_import_accounts, |div| div.child(Checkbox::new("accounts").label("Import Accounts")
                    .checked(self.import_accounts)
                    .on_click(cx.listener(|page, checked, _, _| {
                    page.import_accounts = *checked;
                }))))
                .child(Checkbox::new("instances").label("Import Instances")
                    .checked(self.import_instances)
                    .on_click(cx.listener(|page, checked, _, _| {
                    page.import_instances = *checked;
                })))
                .when(self.import_instances, |d| d.child(div()
                    .w_full()
                    .border_1()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .border_color(cx.theme().border)
                    .max_h_64()
                    .child(v_flex().overflow_y_scrollbar().children(
                        import.paths.iter().map(|path| {
                            SharedString::new(path.to_string_lossy())
                        })
                    )))
                )
                .child(Button::new("doimport").disabled(!import_accounts && !self.import_instances).success().label(label.clone()).on_click(cx.listener(move |page, _, window, cx| {
                    let modal_action = ModalAction::default();

                    page.backend_handle.send(MessageToBackend::ImportFromOtherLauncher {
                        launcher: import_from,
                        import_accounts: import_accounts,
                        import_instances: page.import_instances,
                        modal_action: modal_action.clone()
                    });

                    let title = SharedString::new(label.clone());
                    crate::modals::generic::show_modal(window, cx, title, "Error importing".into(), modal_action);
                })))
            )
        }

        content
    }
}
