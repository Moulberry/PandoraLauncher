use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use bridge::{handle::BackendHandle, message::MessageToBackend};
use gpui::{prelude::*, *};
use gpui_component::{
    alert::Alert, button::{Button, ButtonGroup, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputEvent, InputState}, select::{Select, SelectDelegate, SelectEvent, SelectItem, SelectState}, skeleton::Skeleton, table::{Table, TableDelegate, TableState}, v_flex, ActiveTheme as _, IconName, IndexPath, Selectable, Sizable, WindowExt
};
use schema::{loader::Loader, version_manifest::{MinecraftVersionManifest, MinecraftVersionType}};
use strum::IntoEnumIterator;

use crate::{
    component::{instance_list::InstanceList, named_dropdown::{NamedDropdown, NamedDropdownItem}, page_path::PagePath, responsive_grid::ResponsiveGrid}, entity::{DataEntities, instance::InstanceEntries, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult}}, interface_config::{InstancesViewMode, InterfaceConfig}, ts, ui
};

pub struct InstancesPage {
    instance_table: Entity<TableState<InstanceList>>,
    view_dropdown: Entity<SelectState<NamedDropdown<InstancesViewMode>>>,

    metadata: Entity<FrontendMetadata>,
    instances: Entity<InstanceEntries>,

    backend_handle: BackendHandle,
}

impl InstancesPage {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance_table = InstanceList::create_table(data, window, cx);
        let view_dropdown = cx.new(|cx| {
            let items = InstancesViewMode::iter().map(|view| {
                NamedDropdownItem { name: view.name(), item: view }
            }).collect::<Vec<_>>();
            let current_view = InterfaceConfig::get(cx).instances_view_mode;
            let row = items.iter().position(|v| v.item == current_view).unwrap_or(0);
            let delegate = NamedDropdown::new(items);
            SelectState::new(delegate, Some(IndexPath::new(row)), window, cx)
        });
        cx.subscribe(&view_dropdown, |_, _, event: &SelectEvent<NamedDropdown<InstancesViewMode>>, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };
            let view = value.item;

            InterfaceConfig::get_mut(cx).instances_view_mode = view;
        }).detach();

        Self {
            instance_table,
            view_dropdown,
            metadata: data.metadata.clone(),
            instances: data.instances.clone(),
            backend_handle: data.backend_handle.clone(),
        }
    }
}

impl Render for InstancesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let create_instance = Button::new("create_instance")
            .success()
            .icon(IconName::Plus)
            .label(ts!("instance.create"))
            .on_click(cx.listener(|this, _, window, cx| {
                crate::modals::create_instance::open_create_instance(this.metadata.clone(), this.instances.clone(),
                    this.backend_handle.clone(), window, cx);
            }));
        let select_view = Select::new(&self.view_dropdown).title_prefix(format!("{}: ", ts!("instance.view")));

        let content = match InterfaceConfig::get(cx).instances_view_mode {
            InstancesViewMode::Cards => {
                let cards = self.instance_table.update(cx, |table, cx| {
                    let rows = table.delegate().rows_count(cx);
                    (0..rows).map(|i| table.delegate().render_card(i, cx)).collect::<Vec<_>>()
                });

                let size = Size::new(
                    gpui::AvailableSpace::MinContent,
                    gpui::AvailableSpace::MinContent
                );

                div().p_4().child(ResponsiveGrid::new(size).size_full().gap_4().children(cards)).into_any_element()
            },
            InstancesViewMode::List => {
                Table::new(&self.instance_table).bordered(false).into_any_element()
            },
        };

        let title_buttons = h_flex().gap_3().child(create_instance).child(select_view);

        ui::page(cx, h_flex().gap_8().child(ts!("instance.title")).child(title_buttons))
            .child(content)
    }
}

#[derive(Default)]
pub struct VersionList {
    pub versions: Vec<SharedString>,
    pub matched_versions: Vec<SharedString>,
}

impl SelectDelegate for VersionList {
    type Item = SharedString;

    fn items_count(&self, _section: usize) -> usize {
        self.matched_versions.len()
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.matched_versions.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: gpui_component::select::SelectItem<Value = V>,
        V: PartialEq,
    {
        for (ix, item) in self.matched_versions.iter().enumerate() {
            if item.value() == value {
                return Some(IndexPath::default().row(ix));
            }
        }

        None
    }

    fn perform_search(&mut self, query: &str, _window: &mut Window, _: &mut Context<SelectState<Self>>) -> Task<()> {
        let lower_query = query.to_lowercase();

        self.matched_versions = self
            .versions
            .iter()
            .filter(|item| item.to_lowercase().starts_with(&lower_query))
            .cloned()
            .collect();

        Task::ready(())
    }
}
