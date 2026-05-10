use std::{path::Path, sync::Arc};

use bridge::{
    handle::BackendHandle,
    instance::InstanceID,
    message::{ExportCurseforgeOptions, ExportFormat, ExportModrinthOptions, ExportOptions, MessageToBackend},
    modal_action::ModalAction,
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, IndexPath, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputState, NumberInput},
    select::{Select, SelectEvent, SelectState},
    v_flex, h_flex,
};

use crate::{labelled, modals::generic};

struct ExportInstanceModalState {
    instance_id: InstanceID,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    format: ExportFormat,
    format_options: Vec<SharedString>,
    format_select_state: Entity<SelectState<Vec<SharedString>>>,

    include_saves: bool,
    include_mods: bool,
    include_resourcepacks: bool,
    include_configs: bool,
    include_logs: bool,
    include_cache: bool,
    include_synced: bool,

    name_input: Entity<InputState>,
    version_input: Entity<InputState>,

    modrinth_summary_input: Entity<InputState>,

    curseforge_author_input: Entity<InputState>,
    curseforge_recommended_ram_enabled: bool,
    curseforge_recommended_ram_input: Entity<InputState>,
}

impl ExportInstanceModalState {
    pub fn new(
        instance_id: InstanceID,
        instance_name: SharedString,
        backend_handle: BackendHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let format_options = vec![
            SharedString::new_static(t::instance::export::format::zip()),
            SharedString::new_static(t::instance::export::format::modrinth()),
            SharedString::new_static(t::instance::export::format::curseforge()),
        ];
        let format_select_state = cx.new(|cx| {
            let mut state = SelectState::new(format_options.clone(), None, window, cx);
            state.set_selected_index(Some(IndexPath::new(0)), window, cx);
            state
        });
        cx.subscribe(&format_select_state, Self::on_format_selected).detach();

        let version_input = cx.new(|cx| InputState::new(window, cx).placeholder("1.0.0"));
        let name_input = cx.new(|cx| InputState::new(window, cx).default_value(instance_name.clone()));

        let modrinth_summary_input = cx.new(|cx| InputState::new(window, cx).auto_grow(1, 4));

        let curseforge_author_input = cx.new(|cx| InputState::new(window, cx));
        let curseforge_recommended_ram_input = cx.new(|cx| InputState::new(window, cx).default_value("4096"));

        Self {
            instance_id,
            instance_name,
            backend_handle,
            format: ExportFormat::Zip,
            format_options,
            format_select_state,

            include_saves: true,
            include_mods: true,
            include_resourcepacks: true,
            include_configs: true,
            include_logs: false,
            include_cache: false,
            include_synced: false,

            name_input,
            version_input,

            modrinth_summary_input,

            curseforge_author_input,
            curseforge_recommended_ram_enabled: false,
            curseforge_recommended_ram_input,
        }
    }

    fn on_format_selected(
        &mut self,
        _state: Entity<SelectState<Vec<SharedString>>>,
        event: &SelectEvent<Vec<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(value) = event;
        let Some(value) = value else {
            return;
        };
        if let Some(index) = self.format_options.iter().position(|opt| opt == value) {
            self.format = match index {
                0 => ExportFormat::Zip,
                1 => ExportFormat::Modrinth,
                2 => ExportFormat::Curseforge,
                _ => ExportFormat::Zip,
            };
            cx.notify();
        }
    }

    fn build_options(&self, cx: &mut App) -> ExportOptions {
        let modrinth_summary = self.modrinth_summary_input.read(cx).value();
        let modrinth_summary = if modrinth_summary.trim_ascii().is_empty() {
            None
        } else {
            Some(Arc::<str>::from(modrinth_summary.as_str()))
        };

        let curseforge_author = self.curseforge_author_input.read(cx).value();
        let curseforge_author = if curseforge_author.trim_ascii().is_empty() {
            None
        } else {
            Some(Arc::<str>::from(curseforge_author.as_str()))
        };

        let recommended_ram = if self.curseforge_recommended_ram_enabled {
            self.curseforge_recommended_ram_input.read(cx).value().trim().parse::<u32>().ok()
        } else {
            None
        };

        let version = self.version_input.read(cx).value();
        let mut version = version.as_str().trim_ascii();
        if version.is_empty() {
            version = "1.0.0";
        }

        let name = self.name_input.read(cx).value();
        let name = name.as_str().trim_ascii();

        ExportOptions {
            include_saves: self.include_saves,
            include_mods: self.include_mods,
            include_resourcepacks: self.include_resourcepacks,
            include_configs: self.include_configs,
            include_logs: self.include_logs,
            include_cache: self.include_cache,
            include_synced: self.include_synced,
            modrinth: ExportModrinthOptions {
                name: name.into(),
                version: version.into(),
                summary: modrinth_summary,
            },
            curseforge: ExportCurseforgeOptions {
                name: name.into(),
                version: version.into(),
                author: curseforge_author,
                recommended_ram,
            },
        }
    }

    pub fn render(&mut self, dialog: gpui_component::dialog::Dialog, _window: &mut Window, cx: &mut Context<Self>) -> gpui_component::dialog::Dialog {
        let format_group = Select::new(&self.format_select_state);

        let common_options = v_flex()
            .gap_2()
            .child(Checkbox::new("include_saves")
                .checked(self.include_saves)
                .label(t::instance::export::include_saves())
                .on_click(cx.listener(|this, value, _, cx| { this.include_saves = *value; cx.notify(); })))
            .child(Checkbox::new("include_mods")
                .checked(self.include_mods)
                .label(t::instance::export::include_mods())
                .on_click(cx.listener(|this, value, _, cx| { this.include_mods = *value; cx.notify(); })))
            .child(Checkbox::new("include_resourcepacks")
                .checked(self.include_resourcepacks)
                .label(t::instance::export::include_resourcepacks())
                .on_click(cx.listener(|this, value, _, cx| { this.include_resourcepacks = *value; cx.notify(); })))
            .child(Checkbox::new("include_configs")
                .checked(self.include_configs)
                .label(t::instance::export::include_configs())
                .on_click(cx.listener(|this, value, _, cx| { this.include_configs = *value; cx.notify(); })))
            .child(Checkbox::new("include_logs")
                .checked(self.include_logs)
                .label(t::instance::export::include_logs())
                .on_click(cx.listener(|this, value, _, cx| { this.include_logs = *value; cx.notify(); })))
            .child(Checkbox::new("include_cache")
                .checked(self.include_cache)
                .label(t::instance::export::include_cache())
                .on_click(cx.listener(|this, value, _, cx| { this.include_cache = *value; cx.notify(); })));
        let common_options = common_options.child(Checkbox::new("include_synced")
            .checked(self.include_synced)
            .label(t::instance::export::include_synced())
            .on_click(cx.listener(|this, value, _, cx| { this.include_synced = *value; cx.notify(); })));

        let modrinth_options = v_flex()
            .gap_2()
            .child(labelled(t::instance::export::name(), Input::new(&self.name_input)))
            .child(labelled(t::instance::export::version(), Input::new(&self.version_input)))
            .child(labelled(t::instance::export::summary(), Input::new(&self.modrinth_summary_input)));

        let curseforge_options = v_flex()
            .gap_2()
            .child(labelled(t::instance::export::name(), Input::new(&self.name_input)))
            .child(labelled(t::instance::export::version(), Input::new(&self.version_input)))
            .child(labelled(t::instance::export::author(), Input::new(&self.curseforge_author_input)))
            .child(h_flex()
                .gap_2()
                .child(Checkbox::new("curseforge_ram")
                    .checked(self.curseforge_recommended_ram_enabled)
                    .label(t::instance::export::recommended_ram())
                    .on_click(cx.listener(|this, value, _, cx| {
                        this.curseforge_recommended_ram_enabled = *value;
                        cx.notify();
                    })))
                .child(NumberInput::new(&self.curseforge_recommended_ram_input)
                    .small()
                    .suffix("MiB")
                    .disabled(!self.curseforge_recommended_ram_enabled))
            );

        let content = v_flex()
            .gap_3()
            .child(labelled(t::instance::export::format::label(), format_group))
            .child(labelled(t::instance::export::options(), common_options))
            .when(self.format == ExportFormat::Modrinth, |this| {
                this.child(labelled(t::instance::export::modrinth_options(), modrinth_options))
            })
            .when(self.format == ExportFormat::Curseforge, |this| {
                this.child(labelled(t::instance::export::curseforge_options(), curseforge_options))
            });

        dialog
            .title(t::instance::export::title())
            .child(content)
            .footer(
                h_flex()
                    .gap_2()
                    .child(Button::new("cancel").label(t::common::cancel()).on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("export")
                        .label(t::instance::export::action())
                        .success()
                        .on_click({
                            let instance_id = self.instance_id;
                            let instance_name = self.instance_name.clone();
                            let backend_handle = self.backend_handle.clone();
                            let format = self.format;
                            let options = self.build_options(cx);
                            move |_, window, cx| {
                                window.close_dialog(cx);

                                let backend_handle = backend_handle.clone();
                                let options = options.clone();
                                let format = format;
                                let instance_id = instance_id;
                                let instance_name = instance_name.clone();

                                let suggested = match format {
                                    ExportFormat::Zip => format!("{}.zip", instance_name),
                                    ExportFormat::Modrinth => format!("{}.mrpack", instance_name),
                                    ExportFormat::Curseforge => format!("{}.zip", instance_name),
                                };

                                let user_dirs = directories::UserDirs::new();
                                let directory = user_dirs.as_ref()
                                    .and_then(directories::UserDirs::desktop_dir)
                                    .unwrap_or(Path::new("."));

                                let receiver = cx.prompt_for_new_path(directory, Some(&suggested));
                                let modal_action = ModalAction::default();
                                generic::show_modal(window, cx, t::instance::export::progress().into(), t::instance::export::error().into(), modal_action.clone());

                                cx.spawn(async move |_| {
                                    let Ok(Ok(Some(mut path))) = receiver.await else {
                                        modal_action.set_finished();
                                        return;
                                    };

                                    let extension = match format {
                                        ExportFormat::Zip => "zip",
                                        ExportFormat::Modrinth => "mrpack",
                                        ExportFormat::Curseforge => "zip",
                                    };
                                    if path.extension().is_none() {
                                        path.set_extension(extension);
                                    }

                                    backend_handle.send(MessageToBackend::ExportInstance {
                                        id: instance_id,
                                        format,
                                        options,
                                        output: path,
                                        modal_action,
                                    });
                                }).detach();
                            }
                        })
                    )
            )
    }
}

pub fn open_export_instance(
    instance_id: InstanceID,
    instance_name: SharedString,
    backend_handle: BackendHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let state = cx.new(|cx| {
        ExportInstanceModalState::new(instance_id, instance_name, backend_handle, window, cx)
    });

    window.open_dialog(cx, move |modal, window, cx| {
        cx.update_entity(&state, |state, cx| {
            state.render(modal, window, cx)
        })
    });
}
