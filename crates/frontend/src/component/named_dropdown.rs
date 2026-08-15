use gpui::{prelude::*, *};
use gpui_component::{
    IndexPath,
    select::{SelectDelegate, SelectItem, SelectState},
};

#[derive(Clone)]
pub struct NamedDropdownItem<T: Clone + PartialEq> {
    pub name: SharedString,
    pub item: T
}

impl <T: Clone + PartialEq> PartialEq for NamedDropdownItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
    }
}

impl<T: Clone + PartialEq> SelectItem for NamedDropdownItem<T> {
    type Value = T;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.item
    }
}

pub struct NamedDropdown<T: Clone + PartialEq> {
    items: Vec<NamedDropdownItem<T>>,
}

impl<T: Clone + PartialEq> NamedDropdown<T> {
    pub fn new(items: Vec<NamedDropdownItem<T>>) -> Self {
        Self {
            items
        }
    }

    pub fn create(items: Vec<NamedDropdownItem<T>>, window: &mut Window, cx: &mut App) -> Entity<SelectState<Self>> {
        cx.new(|cx| {
            let delegate = Self::new(items);
            SelectState::new(delegate, None, window, cx)
        })
    }

    pub fn create_and_select(items: Vec<NamedDropdownItem<T>>, selected: T, window: &mut Window, cx: &mut App) -> Entity<SelectState<Self>> {
        cx.new(|cx| {
            let delegate = Self::new(items);
            let mut select_state = SelectState::new(delegate, None, window, cx);
            select_state.set_selected_value(&selected, window, cx);
            select_state
        })
    }
}

impl<T: Clone + PartialEq + 'static> SelectDelegate for NamedDropdown<T> {
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
        _: &mut App,
    ) -> Task<()> {
        Task::ready(())
    }
}
