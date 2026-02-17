use gpui::{prelude::*, *};
use gpui_component::{
    IndexPath,
    select::{SelectDelegate, SelectItem, SelectState},
};

#[derive(Clone)]
pub struct NamedDropdownItem<T: Clone> {
    pub name: SharedString,
    pub item: T
}

impl<T: Clone> SelectItem for NamedDropdownItem<T> {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

pub struct NamedDropdown<T: Clone> {
    items: Vec<NamedDropdownItem<T>>,
}

impl<T: Clone> NamedDropdown<T> {
    pub fn new(items: Vec<NamedDropdownItem<T>>) -> Self {
        Self {
            items
        }
    }

    pub fn create(items: Vec<NamedDropdownItem<T>>, window: &mut Window, cx: &mut App) -> Entity<SelectState<Self>> {
        cx.new(|cx| {
            let instance_list = Self::new(items);
            SelectState::new(instance_list, None, window, cx)
        })
    }
}

impl<T: Clone> SelectDelegate for NamedDropdown<T> {
    type Item = NamedDropdownItem<T>;

    fn items_count(&self, _section: usize) -> usize {
        self.items.len()
    }

    fn item(&self, ix: gpui_component::IndexPath) -> Option<&Self::Item> {
        self.items.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<gpui_component::IndexPath>
    where
        Self::Item: gpui_component::select::SelectItem<Value = V>,
        V: PartialEq,
    {
        for (ix, item) in self.items.iter().enumerate() {
            if item.value() == value {
                return Some(IndexPath::default().row(ix));
            }
        }

        None
    }

    fn perform_search(
        &mut self,
        _query: &str,
        _window: &mut Window,
        _: &mut Context<SelectState<Self>>,
    ) -> Task<()> {
        Task::ready(())
    }
}
