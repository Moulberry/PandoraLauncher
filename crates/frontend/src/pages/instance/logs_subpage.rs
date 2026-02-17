use std::{path::Path, sync::Arc};

use bridge::{
    handle::BackendHandle, instance::InstanceID, message::{LogFiles, MessageToBackend}
};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, h_flex, select::{Select, SelectEvent, SelectState}, spinner::Spinner, v_flex, ActiveTheme as _, Sizable
};

use crate::{component::{named_dropdown::{NamedDropdown, NamedDropdownItem}, readonly_text_field::{ReadonlyTextField, ReadonlyTextFieldWithControls}}, entity::instance::InstanceEntry, root, ts};

pub struct InstanceLogsSubpage {
    instance: InstanceID,
    backend_handle: BackendHandle,
    log_content: Option<Entity<ReadonlyTextFieldWithControls>>,
    no_available_logs: bool,
    available_logs: Option<Entity<SelectState<NamedDropdown<Arc<Path>>>>>,
    clean_old_logs_text: Option<SharedString>,
    last_selected_path: Option<Arc<Path>>,
    _read_log_task: Option<Task<()>>,
    _get_log_files_task: Task<()>,
    _dropdown_change_subscrption: Option<Subscription>,
}

impl InstanceLogsSubpage {
    pub fn new(
        instance: &Entity<InstanceEntry>,
        backend_handle: BackendHandle,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let instance = instance.read(cx);
        let instance_id = instance.id;

        let mut this = Self {
            instance: instance_id,
            backend_handle,
            log_content: None,
            no_available_logs: false,
            available_logs: None,
            clean_old_logs_text: None,
            last_selected_path: None,
            _read_log_task: None,
            _get_log_files_task: Task::ready(()),
            _dropdown_change_subscrption: None,
        };

        this.get_log_files(window, cx);

        this
    }
}

impl InstanceLogsSubpage {
    pub fn get_log_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.no_available_logs = false;
        self.log_content = None;
        self.available_logs = None;
        self.clean_old_logs_text = None;
        self.last_selected_path = None;
        self._read_log_task = None;
        self._dropdown_change_subscrption = None;

        let (send, recv) = tokio::sync::oneshot::channel();
        self._get_log_files_task = cx.spawn_in(window, async move |page, cx| {
            let result: LogFiles = recv.await.unwrap_or_else(|_| LogFiles { paths: Vec::new(), total_gzipped_size: 0 });
            let _ = page.update_in(cx, move |page, window, cx| {
                if result.paths.is_empty() {
                    page.no_available_logs = true;
                } else {
                    let items = result.paths.into_iter().filter_map(|path| {
                        Some(NamedDropdownItem {
                            name: SharedString::new(Arc::from(path.file_name()?.to_string_lossy())),
                            item: path,
                        })
                    }).collect();

                    let dropdown = NamedDropdown::create(items, window, cx);

                    let _dropdown_change_subscrption = cx.subscribe_in(&dropdown, window, move |page, entity, _: &SelectEvent<NamedDropdown<Arc<Path>>>, window, cx| {
                        let selected = entity.read(cx).selected_value().map(|item| item.item.clone());

                        if selected == page.last_selected_path {
                            return;
                        }
                        page.last_selected_path = selected.clone();

                        if let Some(selected) = selected {
                            let (send, mut recv) = tokio::sync::mpsc::channel::<Arc<str>>(256);

                            let text_field = cx.new(move |_| ReadonlyTextField::default());

                            let text_field2 = text_field.clone();
                            page._read_log_task = Some(cx.spawn(async move |_, cx| {
                                while let Some(message) = recv.recv().await {
                                    let _ = cx.update_entity(&text_field2, |text_field, _| {
                                        text_field.add(message);
                                    });
                                }
                                let _ = cx.update_entity(&text_field2, |text_field, _| {
                                    text_field.shrink_to_fit();
                                });
                            }));

                            page.backend_handle.send(MessageToBackend::ReadLog {
                                path: selected.clone(),
                                send,
                            });

                            let backend_handle = page.backend_handle.clone();
                            page.log_content = Some(cx.new(move |cx| {
                                ReadonlyTextFieldWithControls::new(text_field, Box::new(move |div| {
                                    let backend_handle = backend_handle.clone();
                                    let selected = selected.clone();
                                    div.child(Button::new("upload").label(ts!("instance.logs.upload.label")).on_click(move |_, window, cx| {
                                        root::upload_log_file(selected.clone(), &backend_handle, window, cx);
                                    }))
                                }), window, cx)
                            }));
                        } else {
                            page._read_log_task = None;
                            page.log_content = None;
                        }

                        cx.notify();
                    });

                    page._dropdown_change_subscrption = Some(_dropdown_change_subscrption);
                    page.available_logs = Some(dropdown);

                    if result.total_gzipped_size > 0 {
                        let bytes = result.total_gzipped_size;
                        let string = if bytes < 1000 {
                            ts!("instance.logs.cleanup", num = format!("{} bytes", bytes))
                        } else if bytes < 1000*1000 {
                            ts!("instance.logs.cleanup", num = format!("{}kB", bytes/1000))
                        } else if bytes < 1000*1000*1000 {
                            ts!("instance.logs.cleanup", num = format!("{}MB", bytes/1000/1000))
                        } else {
                            ts!("instance.logs.cleanup", num = format!("{}GB", bytes/1000/1000/1000))
                        };
                        page.clean_old_logs_text = Some(string.into());
                    }
                }
                cx.notify();
            });
        });

        self.backend_handle.send(MessageToBackend::GetLogFiles {
            instance: self.instance,
            channel: send,
        });
    }
}

impl Render for InstanceLogsSubpage {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();

        let mut header = h_flex()
            .gap_3()
            .mb_1()
            .ml_1()
            .child(div().text_lg().child(ts!("instance.logs.title")));

        let mut content = div()
            .size_full()
            .border_1()
            .rounded(theme.radius)
            .border_color(theme.border);

        if self.no_available_logs {
            content = content.child(h_flex().justify_center().size_full().text_lg().child(ts!("instance.logs.none")));
        } else {
            if let Some(available_logs) = self.available_logs.as_ref() {
                header = header.child(Select::new(&available_logs).small().mt_0p5().placeholder(ts!("instance.logs.select_file")));
            } else {
                content = content.child(h_flex().justify_center().size_full().text_lg().gap_3().child(ts!("instance.logs.loading")).child(Spinner::new()));
            }

            if let Some(log_content) = self.log_content.clone() {
                content = content.child(log_content);
            } else if self.available_logs.is_some() {
                content = content.child(h_flex().justify_center().size_full().text_lg().child(ts!("instance.logs.select_file")));
            }
        }

        if let Some(clean_old_logs_text) = self.clean_old_logs_text.clone() {
            header = header.child(Button::new("cleanold").label(clean_old_logs_text).success().compact().small().on_click({
                let backend_handle = self.backend_handle.clone();
                let instance = self.instance.clone();
                cx.listener(move |this, _, window, cx| {
                    backend_handle.send(MessageToBackend::CleanupOldLogFiles { instance });
                    this.get_log_files(window, cx);
                })
            }));
        }

        v_flex().p_4().size_full().child(header).child(content)
    }
}
