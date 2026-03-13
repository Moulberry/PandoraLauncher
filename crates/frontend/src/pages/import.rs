use std::{path::PathBuf, sync::Arc};

use bridge::{handle::BackendHandle, import::{ImportFromOtherLauncher, ImportFromOtherLaunchers, ImportStatus, OtherLauncher}, install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget}, message::MessageToBackend, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt, button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, notification::{Notification, NotificationType}, scroll::ScrollableElement, spinner::Spinner, v_flex
};
use log::debug;
use schema::{content::ContentSource, loader::Loader};
use strum::IntoEnumIterator;

use crate::{component::responsive_grid::ResponsiveGrid, entity::DataEntities, icon::PandoraIcon, pages::page::Page, root, ts};

pub struct ImportPage {
    backend_handle: BackendHandle,
    import_from_other_launchers: Option<ImportFromOtherLaunchers>,
    import_details: Option<ImportFromOtherLauncher>,
    failed_details: bool,
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
            import_details: None,
            failed_details: false,
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

    pub fn request_custom_paths(&mut self, cx: &mut Context<Self>, path: PathBuf) {
    	let (send, recv) = tokio::sync::oneshot::channel();
     	cx.spawn(async move |page, cx| {
      		let result: Option<ImportFromOtherLauncher> = recv.await.unwrap_or_default();
        	let _ = page.update(cx, move |page, cx| {
                page.failed_details = result.is_none();
         		page.import_details = result;
           		cx.notify();
         	});
      	}).detach();

      	self.backend_handle.send(MessageToBackend::GetImportFromCustomLauncherPath { channel: send, path });
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(imports) = &self.import_from_other_launchers else {
            let content = v_flex().size_full().p_3().gap_3()
                .child(Spinner::new().with_size(gpui_component::Size::Large));

            return content;
        };

        if self.failed_details {
            self.failed_details = false;
            let notification: Notification = (NotificationType::Error, ts!("import.failed")).into();
            window.push_notification(notification.autohide(true), cx);
        }

        let mut content = v_flex().size_full().p_3().gap_3()
            .child(ResponsiveGrid::new(Size::new(AvailableSpace::MinContent, AvailableSpace::MinContent))
                .gap_2()
                .children({
                    OtherLauncher::iter().map(|launcher| {
                        Button::new(launcher.to_string())
                             .label(format!("Import from {}", launcher))
                             .w_full()
                             .disabled(imports.imports[launcher].is_none())
                             .on_click(cx.listener(move |page, _, _, _| {
                                 page.import_details = page.import_from_other_launchers.as_ref().unwrap().imports[launcher].clone();
                             }))
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
               	.child(Button::new("custom")
                    .label("Import From Custom Directory")
                    .w_full()
                    .on_click(cx.listener(|page, _, window, cx| {
	                    let receiver = cx.prompt_for_paths(PathPromptOptions {
	                        files: false,
	                        directories: true,
	                        multiple: false,
	                        prompt: Some("Select Directory To Import From".into())
	                    });

						let page_entity = cx.entity();
						page._open_file_task = window.spawn(cx, async move |cx| {
    						let Ok(Ok(Some(path))) = receiver.await else {
    						    return;
    						};
                            // we just care about an owned version and not the iter being useless. We can only select one anyway...
                            let Some(dir) = path.into_iter().nth(0) else { return; };

                            _ = page_entity.update_in(cx, |page, _, cx| {
                                page.request_custom_paths(cx, dir);
                            });
						});
                    })))
            );

        if let Some(import) = &self.import_details {
            let import_from = import.launcher;
           	let label = match import.custom_import {
                true => ts!("import.dir.custom", launcher = import_from),
                false => ts!("import.dir.normal", launcher = import_from),
            };
            let import_accounts = self.import_accounts && import.account.is_some();

            // this is just to always make sure it's in alphabetical order.
            // makes it more reliable upon loading as well.
           	let mut list = import.instances.iter().collect::<Vec<_>>();
            list.sort_by(|a, b| a.0.cmp(b.0));
            // println!("{:#?}", list);

            let can_import = import_accounts ||
                (self.import_instances && import.instances.iter().any(|(_, status)| *status == ImportStatus::Importing));

            content = content.child(v_flex()
                .w_full()
                .border_1()
                .gap_2()
                .p_2()
                .rounded(cx.theme().radius_lg)
                .border_color(cx.theme().border)
                .when(import.account.is_some(), |div| div.child(Checkbox::new("accounts").label("Import Accounts")
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
                    .child(h_flex().children([
                    	Button::new("uncheck_all").label("Uncheck All")
                        	.on_click(cx.listener(move |page, _, _, _| {
                       			if let Some(details) = page.import_details.as_mut() {
                           			details.instances.iter_mut().for_each(|(_, state)| state.disable());
                          		}
                        	})),
                    	Button::new("check_all").label("Check All")
                        	.on_click(cx.listener(move |page, _, _, _| {
                       			if let Some(details) = page.import_details.as_mut() {
                           			details.instances.iter_mut().for_each(|(_, state)| state.enable());
                          		}
                        	}))
                    ]))
                    .child(v_flex().overflow_y_scrollbar().children({
                        list.iter().map(|(path, checked)| {
                        	let mut line = v_flex().child(Checkbox::new(SharedString::new(path.to_string_lossy()))
	                         	.label(SharedString::new(path.to_string_lossy()))
	                           	.checked(**checked == ImportStatus::Importing)
	                            .disabled(**checked == ImportStatus::Duplicate)
	                            .on_click({
	                                let path_buf = path.to_path_buf();
	                                cx.listener(move |page, _, _, _| {
	                                    if let Some(details) = page.import_details.as_mut() {
											if let Some(state) = details.instances.get_mut(&path_buf) {
                                                state.flip();
                                            }
                                        }
	                                })
	                            }));
                         	if **checked == ImportStatus::Duplicate {
                         		line = line.child(h_flex().text_color(cx.theme().red).pl_8()
                           			.child(PandoraIcon::TriangleAlert)
                            		.child(ts!("import.duplicated", name = path.file_name().unwrap().to_string_lossy()))
                           		);
                          	}
                           	line
                        })
                    })))
                )
                .child(Button::new("doimport").disabled(!can_import).success().label(label.clone())
                    .tooltip({
                        match can_import {
                            true => ts!("import.enabled", launcher = import_from.to_string()),
                            false => ts!("import.disabled", launcher = import_from.to_string()),
                        }
                    })
                    .on_click(cx.listener(move |page, _, window, cx| {
                        let modal_action = ModalAction::default();
                        debug!("{:?}", page.import_details);

                        let mut details = page.import_details.as_ref().unwrap().clone();
                        details.account = if import_accounts { details.account } else { None };
                        if !page.import_instances { details.instances.clear(); }

                        page.backend_handle.send(MessageToBackend::ImportFromOtherLauncher {
                           	details,
                            modal_action: modal_action.clone()
                        });

                        let title = SharedString::new(label.clone());
                        crate::modals::generic::show_modal(window, cx, title, "Error importing".into(), modal_action);
                        page.import_details = None;
                        // might be a tad bit over-the-top for what we technically need...
                        page.update_launcher_paths(cx);
                    }))
                )
            )
        }

        content
    }
}
