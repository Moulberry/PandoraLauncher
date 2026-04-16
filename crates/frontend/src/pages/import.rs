use std::{path::Path, sync::Arc};

use bridge::{handle::BackendHandle, import::{ImportFromOtherLauncherJob, OtherLauncher}, install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget}, message::MessageToBackend, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Disableable, button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, scroll::ScrollableElement, spinner::Spinner, v_flex
};
use rustc_hash::FxHashSet;
use schema::{content::ContentSource, loader::Loader};
use strum::IntoEnumIterator;

use crate::{component::{path_label::PathLabel, responsive_grid::ResponsiveGrid}, entity::{DataEntities, instance::InstanceEntries}, icon::PandoraIcon, pages::page::Page, root};

pub struct ImportPage {
    backend_handle: BackendHandle,
    instances: Entity<InstanceEntries>,
    import_from: Option<OtherLauncher>,
    import_from_path: Option<PathLabel>,
    import_job: Option<ImportFromOtherLauncherJob>,
    disabled_due_to_name_conflict: FxHashSet<Arc<Path>>,
    disabled_manually: FxHashSet<Arc<Path>>,
    import_accounts: bool,
    import_instances: bool,
    _get_import_job_task: Task<()>,
    _open_file_task: Task<()>,
}

impl ImportPage {
    pub fn new(data: &DataEntities, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            backend_handle: data.backend_handle.clone(),
            instances: data.instances.clone(),
            import_from: None,
            import_from_path: None,
            import_job: None,
            disabled_due_to_name_conflict: FxHashSet::default(),
            disabled_manually: FxHashSet::default(),
            import_accounts: true,
            import_instances: true,
            _get_import_job_task: Task::ready(()),
            _open_file_task: Task::ready(()),
        }
    }

    pub fn get_import_job(&mut self, launcher: OtherLauncher, path: Arc<Path>, cx: &mut Context<Self>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        self._get_import_job_task = cx.spawn(async move |page, cx| {
            let result: Option<ImportFromOtherLauncherJob> = recv.await.unwrap_or_default();
            let _ = page.update(cx, move |page, cx| {
                page.import_job = result;
                page.disabled_due_to_name_conflict.clear();
                page.disabled_manually.clear();

                if let Some(import_job) = &page.import_job {
                    let instances = page.instances.read(cx);
                    let mut instance_file_names = FxHashSet::default();
                    for entry in instances.entries.values() {
                        let entry = entry.read(cx);
                        if let Some(file_name) = entry.root_path.file_name() {
                            instance_file_names.insert(file_name.to_os_string());
                        }
                    }

                    for path in &import_job.paths {
                        let Some(file_name) = path.file_name() else {
                            continue;
                        };
                        if instance_file_names.contains(file_name) {
                            page.disabled_due_to_name_conflict.insert(path.clone());
                        }
                    }
                }

                cx.notify();
            });
        });

        self.backend_handle.send(MessageToBackend::GetImportFromOtherLauncherJob {
            channel: send,
            launcher,
            path,
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
        let mut content = v_flex().size_full().p_3().gap_3()
            .child(ResponsiveGrid::new(Size::new(AvailableSpace::MinContent, AvailableSpace::MinContent))
                .gap_2()
                .children({
                    OtherLauncher::iter().map(|launcher| {
                        Button::new(launcher.name())
                             .label(format!("Import from {}", launcher.name()))
                             .w_full()
                             .on_click(cx.listener(move |page, _, _, cx| {
                                 page.import_from = Some(launcher);

                                 let Some(base_dirs) = directories::BaseDirs::new() else {
                                     page.import_from_path = None;
                                     page.import_job = None;
                                     page._get_import_job_task = Task::ready(());
                                     return;
                                 };

                                 let default_path = launcher.default_path(&base_dirs);
                                 page.import_from_path = Some(PathLabel::new(default_path.clone(), true));
                                 page.import_job = None;
                                 page.get_import_job(launcher, default_path, cx);
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
            );

        if let Some(import_from) = self.import_from {
            let label = format!("Import From {}", import_from.name());

            let mut import_box = v_flex()
                .w_full()
                .border_1()
                .gap_2()
                .p_2()
                .rounded(cx.theme().radius_lg)
                .border_color(cx.theme().border);

            let pick_folder = cx.listener(move |_, _: &ClickEvent, _, cx| {
                let receiver = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Select launcher folder".into()),
                });
                cx.spawn(async move |page, cx| {
                    let Ok(Ok(Some(mut paths))) = receiver.await else {
                        return;
                    };
                    if paths.is_empty() {
                        return;
                    }
                    let path: Arc<Path> = paths.remove(0).into();
                    _ = page.update(cx, |page, cx| {
                        page.import_from_path = Some(PathLabel::new(path.clone(), true));
                        page.import_job = None;
                        page.get_import_job(import_from, path, cx);
                    });
                }).detach();
            });

            if let Some(path) = &self.import_from_path {
                import_box = import_box
                    .child(path.button("select-folder").on_click(pick_folder));
            } else {
                import_box = import_box
                    .child(Button::new("select-folder").success().label("Select Folder").on_click(pick_folder));
            }

            if let Some(import_job) = &self.import_job {
                import_box = import_box.child(h_flex()
                    .gap_2()
                    .text_color(cx.theme().success_foreground)
                    .child(PandoraIcon::Check)
                    .child("Detected launcher files")
                );
                if import_job.import_accounts {
                    import_box = import_box.child(Checkbox::new("accounts").label("Import Accounts")
                        .checked(self.import_accounts)
                        .on_click(cx.listener(|page, checked, _, _| {
                            page.import_accounts = *checked;
                        }))
                    );
                }
                import_box = import_box.child(Checkbox::new("instances").label("Import Instances")
                    .checked(self.import_instances)
                    .on_click(cx.listener(|page, checked, _, _| {
                    page.import_instances = *checked;
                })));
                if self.import_instances {
                    import_box = import_box.child(div()
                        .w_full()
                        .border_1()
                        .p_2()
                        .rounded(cx.theme().radius)
                        .border_color(cx.theme().border)
                        .max_h_64()
                        .child(v_flex().overflow_y_scrollbar().gap_2().children(
                            import_job.paths.iter().enumerate().map(|(index, path)| {
                                if self.disabled_due_to_name_conflict.contains(&*path) {
                                    h_flex()
                                        .gap_4()
                                        .child(Checkbox::new(index).checked(false).disabled(true).label(&*path.to_string_lossy()))
                                        .child(h_flex()
                                            .gap_2()
                                            .line_height(rems(1.0))
                                            .text_color(cx.theme().warning_foreground)
                                            .child(PandoraIcon::TriangleAlert)
                                            .child("Already exists")
                                        ).into_any_element()
                                } else {
                                    Checkbox::new(index)
                                        .checked(!self.disabled_manually.contains(&*path))
                                        .label(&*path.to_string_lossy())
                                        .on_click({
                                            let path = path.clone();
                                            cx.listener(move |page, value, _, _| {
                                                if *value {
                                                    page.disabled_manually.remove(&*path);
                                                } else {
                                                    page.disabled_manually.insert(path.clone());
                                                }
                                            })
                                        })
                                        .into_any_element()
                                }
                            })
                        )))
                }
                let import_accounts = import_job.import_accounts && self.import_accounts;
                let can_import = import_accounts ||
                	(self.import_instances && self.disabled_due_to_name_conflict.len() + self.disabled_manually.len() != import_job.paths.len());
                import_box = import_box.child(Button::new("doimport")
                    .tooltip(match can_import {
                        true => t::import::enabled(import_from.name()),
                        false => t::import::disabled(import_from.name()),
                    })
                    .disabled(!can_import)
                    .success()
                    .label(label.clone())
                    .on_click(cx.listener(move |page, _, window, cx| {
                        let Some(import_job) = &page.import_job else {
                            return;
                        };

                        let modal_action = ModalAction::default();

                        page.backend_handle.send(MessageToBackend::ImportFromOtherLauncher {
                            launcher: import_from,
                            import_job: ImportFromOtherLauncherJob {
                                import_accounts,
                                root: import_job.root.clone(),
                                paths: import_job.paths.iter().cloned().filter(|path| {
                                    !page.disabled_due_to_name_conflict.contains(&*path)
                                        && !page.disabled_manually.contains(&*path)
                                }).collect()
                            },
                            modal_action: modal_action.clone()
                        });

                        let title = SharedString::new(label.clone());
                        crate::modals::generic::show_modal(window, cx, title, "Error importing".into(), modal_action);
                    }))
                );
            } else if self._get_import_job_task.is_ready() {
                import_box = import_box.child(h_flex()
                    .gap_2()
                    .text_color(cx.theme().danger_foreground)
                    .child(PandoraIcon::TriangleAlert)
                    .child("Unable to detect launcher files")
                );
            } else {
                import_box = import_box.child(h_flex()
                    .gap_2()
                    .child(Spinner::new())
                    .child("Loading launcher data...")
                );
            }

            content = content.child(import_box);
        }

        content
    }
}
