use bridge::handle::BackendHandle;
use gpui::{prelude::*, *};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    table::{Column, ColumnSort, TableDelegate, TableState},
};

use crate::{
    entity::{
        instance::{InstanceAddedEvent, InstanceEntry, InstanceModifiedEvent, InstanceRemovedEvent}, DataEntities
    }, pages::instance::instance_page::InstanceSubpageType, root, ui
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
            let _instance_added_subscription = cx.subscribe::<_, InstanceAddedEvent>(&instances, |table: &mut TableState<InstanceList>, _, event, cx| {
                table.delegate_mut().items.insert(0, event.instance.clone());
                cx.notify();
            });
            let _instance_removed_subscription = cx.subscribe::<_, InstanceRemovedEvent>(&instances, |table, _, event, cx| {
                table.delegate_mut().items.retain(|instance| {
                    instance.id != event.id
                });
                cx.notify();
            });
            let _instance_modified_subscription = cx.subscribe::<_, InstanceModifiedEvent>(&instances, |table, _, event, cx| {
                if let Some(entry) = table.delegate_mut().items.iter_mut().find(|entry| entry.id == event.instance.id) {
                    *entry = event.instance.clone();
                    cx.notify();
                }
            });
            let instance_list = Self {
                columns: vec![
                    Column::new("controls", "")
                        .width(150.)
                        .fixed_left()
                        .movable(false)
                        .resizable(false),
                    Column::new("name", "Name")
                        .width(150.)
                        .fixed_left()
                        .sortable()
                        .resizable(true),
                    Column::new("version", "Version")
                        .width(150.)
                        .fixed_left()
                        .sortable()
                        .resizable(true),
                    Column::new("loader", "Loader")
                        .width(150.)
                        .fixed_left()
                        .resizable(true),
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
                    ColumnSort::Descending => lexical_sort::natural_lexical_cmp(&a.configuration.minecraft_version, &b.configuration.minecraft_version).reverse(),
                    _ => lexical_sort::natural_lexical_cmp(&a.configuration.minecraft_version, &b.configuration.minecraft_version),
                }),
                _ => {},
            }
        }
    }

    fn render_td(&mut self, row_ix: usize, col_ix: usize, _window: &mut Window, _cx: &mut Context<TableState<Self>>) -> impl IntoElement {
        let item = &self.items[row_ix];
        if let Some(col) = self.columns.get(col_ix) {
            match col.key.as_ref() {
                "name" => item.name.clone().into_any_element(),
                "version" => item.configuration.minecraft_version.as_str().into_any_element(),
                "controls" => {
                    let backend_handle = self.backend_handle.clone();
                    h_flex()
                        .size_full()
                        .gap_2()
                        .border_r_4()
                        .child(Button::new("start").w(relative(0.5)).small().success().label("Start").on_click({
                            let name = item.name.clone();
                            let id = item.id;
                            move |_, window, cx| {
                                root::start_instance(id, name.clone(), None, &backend_handle, window, cx);
                            }
                        }))
                        .child(Button::new("view").w(relative(0.5)).small().info().label("View").on_click({
                            let id = item.id;
                            move |_, window, cx| {
                                root::switch_page(ui::PageType::InstancePage(id, InstanceSubpageType::Quickplay),
                                    &[ui::PageType::Instances], window, cx);
                            }
                        }))
                        .into_any_element()
                },
                "loader" => item.configuration.loader.name().into_any_element(),
                _ => "Unknown".into_any_element(),
            }
        } else {
            "Unknown".into_any_element()
        }
    }
}
