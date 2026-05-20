use bridge::{handle::BackendHandle, instance::InstanceStatus, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    table::{Column, ColumnSort, TableDelegate, TableState},
    v_flex,
};

use crate::{
    entity::{
        DataEntities,
        instance::{InstanceAddedEvent, InstanceEntry, InstanceModifiedEvent, InstanceRemovedEvent},
    },
    interface_config::InterfaceConfig,
    modals, png_render_cache, root, ui,
};

pub struct InstanceList {
    columns: Vec<Column>,
    items: Vec<InstanceEntry>,
    backend_handle: BackendHandle,
    _instance_added_subscription: Subscription,
    _instance_removed_subscription: Subscription,
    _instance_modified_subscription: Subscription,
}

impl InstanceList {
    pub fn create_table(data: &DataEntities, window: &mut Window, cx: &mut App) -> Entity<TableState<Self>> {
        let instances = data.instances.clone();
        let items = instances.read(cx).entries.values().map(|i| i.read(cx).clone()).collect();
        cx.new(|cx| {
            let _instance_added_subscription = cx.subscribe::<_, InstanceAddedEvent>(
                &instances,
                |table: &mut TableState<InstanceList>, _, event, cx| {
                    table.delegate_mut().items.insert(0, event.instance.clone());
                    cx.notify();
                },
            );
            let _instance_removed_subscription =
                cx.subscribe::<_, InstanceRemovedEvent>(&instances, |table, _, event, cx| {
                    table.delegate_mut().items.retain(|instance| instance.id != event.id);
                    cx.notify();
                });
            let _instance_modified_subscription =
                cx.subscribe::<_, InstanceModifiedEvent>(&instances, |table, _, event, cx| {
                    if let Some(entry) =
                        table.delegate_mut().items.iter_mut().find(|entry| entry.id == event.instance.id)
                    {
                        *entry = event.instance.clone();
                        cx.notify();
                    }
                });
            let instance_list = Self {
                columns: vec![
                    Column::new("controls", "").width(150.).fixed_left().movable(false).resizable(false),
                    Column::new("name", t::instance::name())
                        .width(150.)
                        .fixed_left()
                        .sortable()
                        .resizable(true),
                    Column::new("version", t::instance::version())
                        .width(150.)
                        .fixed_left()
                        .sortable()
                        .resizable(true),
                    Column::new("loader", t::instance::modloader()).width(150.).fixed_left().resizable(true),
                    Column::new("remove", "").width(44.).fixed_left().movable(false).resizable(false),
                ],
                items,
                backend_handle: data.backend_handle.clone(),
                _instance_added_subscription,
                _instance_removed_subscription,
                _instance_modified_subscription,
            };
            TableState::new(instance_list, window, cx)
        })
    }

    pub fn render_card(&self, index: usize, cx: &mut App) -> Div {
        let item = &self.items[index];
        let loader_and_version = format!(
            "{} {}",
            item.configuration.loader.pretty_name(),
            item.configuration.minecraft_version.as_str(),
        );

        let icon_element = if let Some(icon) = item.icon.clone() {
            let transform = png_render_cache::ImageTransformation::Resize { width: 64, height: 64 };
            png_render_cache::render_with_transform(icon, transform, cx)
                .rounded(cx.theme().radius)
                .size_16()
                .min_w_16()
                .min_h_16()
                .into_any_element()
        } else {
            let icon_path = item.configuration.instance_fallback_icon.map(|s| s.as_str()).unwrap_or("icons/box.svg");
            Icon::default().path(icon_path).size_16().min_w_16().min_h_16().into_any_element()
        };

        let play_button = render_play_button(item, index, self.backend_handle.clone());

        let theme = cx.theme();
        let id = item.id;
        let name = item.name.clone();
        let backend_handle = self.backend_handle.clone();
        let backend_handle_for_icon = self.backend_handle.clone();
        let backend_handle_for_rename = self.backend_handle.clone();
        let name_for_rename = item.name.clone();
        let trash_icon = Icon::default().path("icons/trash-2.svg");
        let edit_icon = Icon::default().path("icons/brush.svg").text_color(white());
        let icon_hover_group = format!("instance-icon-edit-{index}");
        let icon_overlay_hover_group = icon_hover_group.clone();
        let icon = div()
            .id(("icon", index))
            .group(icon_hover_group)
            .cursor_pointer()
            .size_16()
            .min_w_16()
            .min_h_16()
            .relative()
            .on_click(move |_, window, cx| {
                let backend_handle = backend_handle_for_icon.clone();
                crate::modals::select_icon::open_select_icon(
                    Box::new(move |icon, _| {
                        backend_handle.send(MessageToBackend::SetInstanceIcon { id, icon: Some(icon) });
                    }),
                    window,
                    cx,
                );
            })
            .child(icon_element)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(black().opacity(0.5))
                    .opacity(0.0)
                    .group_hover(icon_overlay_hover_group, |this| this.opacity(1.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(edit_icon.clone().size_8()),
            );

        v_flex()
            .flex_1()
            .p_2()
            .gap_2()
            .w_full()
            .min_w_64()
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius_lg)
            .relative()
            .child(
                Button::new(("remove", index))
                    .absolute()
                    .top_1()
                    .right_1()
                    .danger()
                    .small()
                    .compact()
                    .icon(trash_icon)
                    .on_click(move |click: &ClickEvent, window, cx| {
                        if InterfaceConfig::get(cx).quick_delete_instance && click.modifiers().shift {
                            backend_handle.send(MessageToBackend::DeleteInstance { id });
                        } else {
                            modals::delete_instance::open_delete_instance(
                                id,
                                name.clone(),
                                backend_handle.clone(),
                                window,
                                cx,
                            );
                        }
                    }),
            )
            .child({
                let name_hover_group = format!("instance-name-edit-{index}");
                let name_overlay_hover_group = name_hover_group.clone();
                h_flex().w_full().gap_2().child(icon).child(
                    v_flex()
                        .truncate()
                        .w_full()
                        .relative()
                        .child(
                            div()
                                .id(("rename", index))
                                .group(name_hover_group)
                                .cursor_pointer()
                                .w_full()
                                .pl_5()
                                .on_click(move |_, window, cx| {
                                    modals::rename_instance::open_rename_instance(
                                        id,
                                        name_for_rename.clone(),
                                        backend_handle_for_rename.clone(),
                                        window,
                                        cx,
                                    );
                                })
                                .child(item.name.clone())
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left_0()
                                        .opacity(0.0)
                                        .group_hover(name_overlay_hover_group, |this| this.opacity(1.0))
                                        .flex()
                                        .items_center()
                                        .justify_start()
                                        .child(edit_icon.clone().size_4()),
                                ),
                        )
                        .child(loader_and_version),
                )
            })
            .child(h_flex().gap_2().child(play_button.flex_1().small()).child(
                Button::new(("view", index)).flex_1().small().info().label(t::instance::view()).on_click({
                    let name = item.name.clone();
                    move |_, window, cx| {
                        root::switch_page(
                            ui::PageType::InstancePage { name: name.clone() },
                            &[ui::PageType::Instances],
                            window,
                            cx,
                        );
                    }
                }),
            ))
    }
}

impl TableDelegate for InstanceList {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.items.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> gpui_component::table::Column {
        self.columns[col_ix].clone()
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: gpui_component::table::ColumnSort,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
        if let Some(col) = self.columns.get_mut(col_ix) {
            match col.key.as_ref() {
                "name" => self.items.sort_by(|a, b| match sort {
                    ColumnSort::Descending => lexical_sort::natural_lexical_cmp(&a.name, &b.name).reverse(),
                    _ => lexical_sort::natural_lexical_cmp(&a.name, &b.name),
                }),
                "version" => self.items.sort_by(|a, b| match sort {
                    ColumnSort::Descending => lexical_sort::natural_lexical_cmp(
                        &a.configuration.minecraft_version,
                        &b.configuration.minecraft_version,
                    )
                    .reverse(),
                    _ => lexical_sort::natural_lexical_cmp(
                        &a.configuration.minecraft_version,
                        &b.configuration.minecraft_version,
                    ),
                }),
                _ => {},
            }
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let item = &self.items[row_ix];
        if let Some(col) = self.columns.get(col_ix) {
            match col.key.as_ref() {
                "name" => {
                    let id = item.id;
                    let name = item.name.clone();
                    let backend_handle = self.backend_handle.clone();
                    let edit_icon = Icon::default().path("icons/brush.svg").text_color(white());
                    let hover_group = format!("instance-list-name-edit-{row_ix}");
                    let overlay_hover_group = hover_group.clone();
                    div()
                        .id(("rename-list", row_ix))
                        .group(hover_group)
                        .relative()
                        .cursor_pointer()
                        .w_full()
                        .pl_5()
                        .on_click(move |_, window, cx| {
                            modals::rename_instance::open_rename_instance(
                                id,
                                name.clone(),
                                backend_handle.clone(),
                                window,
                                cx,
                            );
                        })
                        .child(item.name.clone())
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .opacity(0.0)
                                .group_hover(overlay_hover_group, |this| this.opacity(1.0))
                                .flex()
                                .items_center()
                                .justify_start()
                                .child(edit_icon.size_4()),
                        )
                        .into_any_element()
                },
                "version" => item.configuration.minecraft_version.as_str().into_any_element(),
                "controls" => {
                    let play_button = render_play_button(item, row_ix, self.backend_handle.clone());

                    h_flex()
                        .size_full()
                        .gap_2()
                        .border_r_4()
                        .child(play_button.w_1_2().small())
                        .child(Button::new("view").w_1_2().small().info().label(t::instance::view()).on_click({
                            let name = item.name.clone();
                            move |_, window, cx| {
                                root::switch_page(
                                    ui::PageType::InstancePage { name: name.clone() },
                                    &[ui::PageType::Instances],
                                    window,
                                    cx,
                                );
                            }
                        }))
                        .into_any_element()
                },
                "loader" => item.configuration.loader.pretty_name().into_any_element(),
                "remove" => {
                    let backend_handle = self.backend_handle.clone();
                    let id = item.id;
                    let name = item.name.clone();
                    let trash_icon = Icon::default().path("icons/trash-2.svg");
                    h_flex()
                        .size_full()
                        .items_center()
                        .child(Button::new(("remove", row_ix)).danger().small().compact().icon(trash_icon).on_click(
                            move |click: &ClickEvent, window, cx| {
                                if InterfaceConfig::get(cx).quick_delete_instance && click.modifiers().shift {
                                    backend_handle.send(MessageToBackend::DeleteInstance { id });
                                } else {
                                    modals::delete_instance::open_delete_instance(
                                        id,
                                        name.clone(),
                                        backend_handle.clone(),
                                        window,
                                        cx,
                                    );
                                }
                            },
                        ))
                        .into_any_element()
                },
                _ => t::common::unknown().into_any_element(),
            }
        } else {
            t::common::unknown().into_any_element()
        }
    }
}

fn render_play_button(item: &InstanceEntry, index: usize, backend_handle: BackendHandle) -> Button {
    let name = item.name.clone();
    let id = item.id;
    match item.status {
        InstanceStatus::NotRunning => Button::new(("start_instance", index))
            .success()
            .label(t::instance::start::label())
            .on_click(move |_, window, cx| {
                root::start_instance(id, name.clone(), None, &backend_handle, window, cx);
            }),
        InstanceStatus::Launching => Button::new(("launching", index)).warning().label("..."),
        InstanceStatus::Stopping => Button::new(("launching", index)).danger().label("..."),
        InstanceStatus::Running => {
            Button::new(("kill_instance", index)).danger().label(t::instance::kill()).on_click({
                let backend_handle = backend_handle.clone();
                move |_, _, _| {
                    backend_handle.send(MessageToBackend::KillInstance { id });
                }
            })
        },
    }
}
