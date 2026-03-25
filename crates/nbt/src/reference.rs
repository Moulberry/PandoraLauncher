use std::{fmt::Debug, hint::unreachable_unchecked};

use super::{NBT, NBTCompound, NBTNode, TagType, pretty};

#[derive(Copy, Clone, Debug)]
pub enum NBTRef<'a> {
    Byte(&'a i8),
    Short(&'a i16),
    Int(&'a i32),
    Long(&'a i64),
    Float(&'a f32),
    Double(&'a f64),
    ByteArray(&'a Vec<i8>),
    String(&'a String),
    List(ListRef<'a>),
    Compound(CompoundRef<'a>),
    IntArray(&'a Vec<i32>),
    LongArray(&'a Vec<i64>),
}

impl PartialEq for NBTRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Byte(l0), Self::Byte(r0)) => l0 == r0,
            (Self::Short(l0), Self::Short(r0)) => l0 == r0,
            (Self::Int(l0), Self::Int(r0)) => l0 == r0,
            (Self::Long(l0), Self::Long(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => l0 == r0 || (l0.is_nan() && r0.is_nan()),
            (Self::Double(l0), Self::Double(r0)) => l0 == r0 || (l0.is_nan() && r0.is_nan()),
            (Self::ByteArray(l0), Self::ByteArray(r0)) => l0 == r0,
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (Self::List(l0), Self::List(r0)) => l0 == r0,
            (Self::Compound(l0), Self::Compound(r0)) => l0 == r0,
            (Self::IntArray(l0), Self::IntArray(r0)) => l0 == r0,
            (Self::LongArray(l0), Self::LongArray(r0)) => l0 == r0,
            _ => false,
        }
    }
}

macro_rules! as_basic {
    ($name:ident, $value_type:ty, $node:ident) => {
        paste::paste! {
            pub fn [<as_ $name>](self) -> Option<&'a $value_type> {
                match self {
                    NBTRef::$node(value) => Some(value),
                    _ => None,
                }
            }
        }
    };
}

impl<'a> NBTRef<'a> {
    super::enumerate_basic_types!(as_basic);

    pub fn as_compound(self) -> Option<CompoundRef<'a>> {
        match self {
            NBTRef::Compound(compound) => Some(compound),
            _ => None,
        }
    }

    pub fn as_list(self) -> Option<ListRef<'a>> {
        match self {
            NBTRef::List(list) => Some(list),
            _ => None,
        }
    }

    pub fn tag_type(&self) -> TagType {
        match self {
            Self::Byte(_) => super::TAG_BYTE_ID,
            Self::Short(_) => super::TAG_SHORT_ID,
            Self::Int(_) => super::TAG_INT_ID,
            Self::Long(_) => super::TAG_LONG_ID,
            Self::Float(_) => super::TAG_FLOAT_ID,
            Self::Double(_) => super::TAG_DOUBLE_ID,
            Self::ByteArray(_) => super::TAG_BYTE_ARRAY_ID,
            Self::String(_) => super::TAG_STRING_ID,
            Self::List(_) => super::TAG_LIST_ID,
            Self::Compound(_) => super::TAG_COMPOUND_ID,
            Self::IntArray(_) => super::TAG_INT_ARRAY_ID,
            Self::LongArray(_) => super::TAG_LONG_ARRAY_ID,
        }
    }
}

#[derive(Debug)]
pub enum NBTRefMut<'a> {
    Byte(&'a mut i8),
    Short(&'a mut i16),
    Int(&'a mut i32),
    Long(&'a mut i64),
    Float(&'a mut f32),
    Double(&'a mut f64),
    ByteArray(&'a mut Vec<i8>),
    String(&'a mut String),
    List(ListRefMut<'a>),
    Compound(CompoundRefMut<'a>),
    IntArray(&'a mut Vec<i32>),
    LongArray(&'a mut Vec<i64>),
}

macro_rules! as_basic_mut {
    ($name:ident, $value_type:ty, $node:ident) => {
        paste::paste! {
            pub fn [<as_ $name>](&mut self) -> Option<&mut $value_type> {
                match self {
                    NBTRefMut::$node(value) => Some(value),
                    _ => None,
                }
            }
        }
    };
}

impl<'a> NBTRefMut<'a> {
    super::enumerate_basic_types!(as_basic_mut);

    pub fn as_compound(self) -> Option<CompoundRefMut<'a>> {
        match self {
            NBTRefMut::Compound(compound) => Some(compound),
            _ => None,
        }
    }

    pub fn as_list(self) -> Option<ListRefMut<'a>> {
        match self {
            NBTRefMut::List(list) => Some(list),
            _ => None,
        }
    }

    pub fn tag_type(&self) -> TagType {
        match self {
            Self::Byte(_) => super::TAG_BYTE_ID,
            Self::Short(_) => super::TAG_SHORT_ID,
            Self::Int(_) => super::TAG_INT_ID,
            Self::Long(_) => super::TAG_LONG_ID,
            Self::Float(_) => super::TAG_FLOAT_ID,
            Self::Double(_) => super::TAG_DOUBLE_ID,
            Self::ByteArray(_) => super::TAG_BYTE_ARRAY_ID,
            Self::String(_) => super::TAG_STRING_ID,
            Self::List(_) => super::TAG_LIST_ID,
            Self::Compound(_) => super::TAG_COMPOUND_ID,
            Self::IntArray(_) => super::TAG_INT_ARRAY_ID,
            Self::LongArray(_) => super::TAG_LONG_ARRAY_ID,
        }
    }
}

#[derive(Copy, Clone)]
pub struct CompoundRef<'a> {
    pub(crate) nbt: &'a NBT,
    pub(crate) node_idx: usize,
}

impl<'a> Debug for CompoundRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty::to_pretty_debug_compound(f, self)
    }
}

impl PartialEq for CompoundRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        let self_compound = self.get_self_node();
        let other_compound = other.get_self_node();

        if self_compound.0.len() != other_compound.0.len() {
            return false;
        }

        let zipped = self_compound.0.iter().zip(other_compound.0.iter());
        for ((self_child_name, self_child_idx), (other_child_name, other_child_idx)) in zipped {
            if self_child_name != other_child_name {
                return false;
            }
            if self.nbt.get_reference(*self_child_idx) != other.nbt.get_reference(*other_child_idx) {
                return false;
            }
        }

        true
    }
}

impl<'a> CompoundRef<'a> {
    pub fn clone_nbt(&self) -> NBT {
        let mut nbt = NBT::new();
        let mut compound = nbt.as_compound_mut().unwrap();
        for (key, entry) in self.entries() {
            match entry {
                NBTRef::Byte(v) => compound.insert_byte(key, *v),
                NBTRef::Short(v) => compound.insert_short(key, *v),
                NBTRef::Int(v) => compound.insert_int(key, *v),
                NBTRef::Long(v) => compound.insert_long(key, *v),
                NBTRef::Float(v) => compound.insert_float(key, *v),
                NBTRef::Double(v) => compound.insert_double(key, *v),
                NBTRef::ByteArray(v) => compound.insert_byte_array(key, v.clone()),
                NBTRef::String(v) => compound.insert_string(key, v.clone()),
                NBTRef::List(v) => v.clone_into(compound.create_list(key, v.children_type)),
                NBTRef::Compound(v) => v.clone_into(compound.create_compound(key)),
                NBTRef::IntArray(v) => compound.insert_int_array(key, v.clone()),
                NBTRef::LongArray(v) => compound.insert_long_array(key, v.clone()),
            }
        }
        nbt
    }

    fn clone_into(&self, mut into: CompoundRefMut<'_>) {
        for (key, entry) in self.entries() {
            match entry {
                NBTRef::Byte(v) => into.insert_byte(key, *v),
                NBTRef::Short(v) => into.insert_short(key, *v),
                NBTRef::Int(v) => into.insert_int(key, *v),
                NBTRef::Long(v) => into.insert_long(key, *v),
                NBTRef::Float(v) => into.insert_float(key, *v),
                NBTRef::Double(v) => into.insert_double(key, *v),
                NBTRef::ByteArray(v) => into.insert_byte_array(key, v.clone()),
                NBTRef::String(v) => into.insert_string(key, v.clone()),
                NBTRef::List(v) => v.clone_into(into.create_list(key, v.children_type)),
                NBTRef::Compound(v) => v.clone_into(into.create_compound(key)),
                NBTRef::IntArray(v) => into.insert_int_array(key, v.clone()),
                NBTRef::LongArray(v) => into.insert_long_array(key, v.clone()),
            }
        }
    }

    pub(crate) fn get_self_node(&self) -> &NBTCompound {
        match self.nbt.nodes.get(self.node_idx) {
            Some(NBTNode::Compound(compound)) => compound,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn find_idx(&self, key: &str) -> Option<usize> {
        let compound = self.get_self_node();
        compound.find(key)
    }

    fn get_node(&self, idx: usize) -> &NBTNode {
        &self.nbt.nodes[idx]
    }

    pub fn is_empty(&self) -> bool {
        let compound = self.get_self_node();
        compound.0.is_empty()
    }

    pub fn len(&self) -> usize {
        let compound = self.get_self_node();
        compound.0.len()
    }

    pub fn entries(&self) -> CompoundIterator<'_> {
        CompoundIterator {
            nbt: self.nbt,
            compound: self.get_self_node(),
            index: 0,
        }
    }

    // todo: should probably return an iterator instead
    pub fn keys(&self) -> Vec<&str> {
        let mut refs: Vec<&str> = vec![];

        let compound = self.get_self_node();
        for (ele, _) in compound.0.iter() {
            refs.push(ele);
        }

        refs
    }

    super::enumerate_basic_types!(super::find);

    pub fn find_numeric<T: num::FromPrimitive>(&self, key: &str) -> Option<T> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::Byte(v) => T::from_i8(*v),
            NBTNode::Short(v) => T::from_i16(*v),
            NBTNode::Int(v) => T::from_i32(*v),
            NBTNode::Long(v) => T::from_i64(*v),
            NBTNode::Float(v) => T::from_f32(*v),
            NBTNode::Double(v) => T::from_f64(*v),
            NBTNode::ByteArray(_) => None,
            NBTNode::String(_) => None,
            NBTNode::List {
                type_id: _,
                children: _,
            } => None,
            NBTNode::Compound(_) => None,
            NBTNode::IntArray(_) => None,
            NBTNode::LongArray(_) => None,
        }
    }

    pub fn find_list(&self, key: &str, type_id: TagType) -> Option<ListRef<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::List {
                type_id: list_type_id,
                children: _,
            } if *list_type_id == type_id => Some(ListRef {
                nbt: self.nbt,
                node_idx: idx,
                children_type: type_id,
            }),
            _ => None,
        }
    }

    pub fn find_list_of_any(&self, key: &str) -> Option<ListRef<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::List {
                type_id: list_type_id,
                children: _,
            } => Some(ListRef {
                nbt: self.nbt,
                node_idx: idx,
                children_type: *list_type_id,
            }),
            _ => None,
        }
    }

    pub fn find_compound(&self, key: &str) -> Option<CompoundRef<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::Compound(_) => Some(CompoundRef {
                nbt: self.nbt,
                node_idx: idx,
            }),
            _ => None,
        }
    }

    pub fn find(&self, key: &str) -> Option<NBTRef<'_>> {
        let idx = self.find_idx(key)?;
        Some(self.nbt.get_reference(idx))
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.find_idx(key).is_some()
    }
}

pub struct CompoundRefMut<'a> {
    pub(crate) nbt: &'a mut NBT,
    pub(crate) node_idx: usize,
}

impl<'a> Debug for CompoundRefMut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty::to_pretty_debug_compound_mut(f, self)
    }
}

impl<'a> CompoundRefMut<'a> {
    pub fn as_immutable_ref(&self) -> CompoundRef<'_> {
        CompoundRef {
            nbt: self.nbt,
            node_idx: self.node_idx,
        }
    }

    pub(crate) fn get_self_node(&self) -> &NBTCompound {
        match self.nbt.nodes.get(self.node_idx) {
            Some(NBTNode::Compound(compound)) => compound,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn get_self_node_mut(&mut self) -> &mut NBTCompound {
        match self.nbt.nodes.get_mut(self.node_idx) {
            Some(NBTNode::Compound(compound)) => compound,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn insert_node(&mut self, key: &str, node: NBTNode) -> usize {
        let idx = self.nbt.nodes.insert(node);

        let compound = self.get_self_node_mut();
        compound.insert(key, idx);

        idx
    }

    fn find_idx(&self, key: &str) -> Option<usize> {
        let compound = self.get_self_node();
        compound.find(key)
    }

    fn get_node(&self, idx: usize) -> &NBTNode {
        &self.nbt.nodes[idx]
    }

    fn get_node_mut(&mut self, idx: usize) -> &mut NBTNode {
        &mut self.nbt.nodes[idx]
    }

    pub fn is_empty(&self) -> bool {
        let compound = self.get_self_node();
        compound.0.is_empty()
    }

    pub fn entries(&self) -> CompoundIterator<'_> {
        CompoundIterator {
            nbt: self.nbt,
            compound: self.get_self_node(),
            index: 0,
        }
    }

    super::enumerate_basic_types!(super::insert);
    super::enumerate_basic_types!(super::find);
    super::enumerate_basic_types!(super::find_mut);

    pub fn find_numeric<T: num::FromPrimitive>(&self, key: &str) -> Option<T> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::Byte(v) => T::from_i8(*v),
            NBTNode::Short(v) => T::from_i16(*v),
            NBTNode::Int(v) => T::from_i32(*v),
            NBTNode::Long(v) => T::from_i64(*v),
            NBTNode::Float(v) => T::from_f32(*v),
            NBTNode::Double(v) => T::from_f64(*v),
            NBTNode::ByteArray(_) => None,
            NBTNode::String(_) => None,
            NBTNode::List {
                type_id: _,
                children: _,
            } => None,
            NBTNode::Compound(_) => None,
            NBTNode::IntArray(_) => None,
            NBTNode::LongArray(_) => None,
        }
    }

    pub fn create_list(&mut self, key: &str, type_id: TagType) -> ListRefMut<'_> {
        let idx = self.insert_node(
            key,
            NBTNode::List {
                type_id,
                children: Default::default(),
            },
        );

        ListRefMut {
            nbt: self.nbt,
            node_idx: idx,
        }
    }

    pub fn create_compound(&mut self, key: &str) -> CompoundRefMut<'_> {
        let idx = self.insert_node(key, NBTNode::Compound(Default::default()));

        CompoundRefMut {
            nbt: self.nbt,
            node_idx: idx,
        }
    }

    pub fn find_list(&self, key: &str, type_id: TagType) -> Option<ListRef<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::List {
                type_id: list_type_id,
                children: _,
            } if *list_type_id == type_id => Some(ListRef {
                nbt: self.nbt,
                node_idx: idx,
                children_type: type_id,
            }),
            _ => None,
        }
    }

    pub fn find_list_of_any(&self, key: &str) -> Option<ListRef<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::List {
                type_id: list_type_id,
                children: _,
            } => Some(ListRef {
                nbt: self.nbt,
                node_idx: idx,
                children_type: *list_type_id,
            }),
            _ => None,
        }
    }

    pub fn find_compound(&self, key: &str) -> Option<CompoundRef<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::Compound(_) => Some(CompoundRef {
                nbt: self.nbt,
                node_idx: idx,
            }),
            _ => None,
        }
    }

    pub fn find_list_mut(&mut self, key: &str, type_id: TagType) -> Option<ListRefMut<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::List {
                type_id: list_type_id,
                children: _,
            } if *list_type_id == type_id => Some(ListRefMut {
                nbt: self.nbt,
                node_idx: idx,
            }),
            _ => None,
        }
    }

    pub fn find_compound_mut(&mut self, key: &str) -> Option<CompoundRefMut<'_>> {
        let idx = self.find_idx(key)?;
        match self.get_node(idx) {
            NBTNode::Compound(_) => Some(CompoundRefMut {
                nbt: self.nbt,
                node_idx: idx,
            }),
            _ => None,
        }
    }

    pub fn find(&self, key: &str) -> Option<NBTRef<'_>> {
        let idx = self.find_idx(key)?;
        Some(self.nbt.get_reference(idx))
    }

    pub fn find_mut(&mut self, key: &str) -> Option<NBTRefMut<'_>> {
        let idx = self.find_idx(key)?;
        Some(self.nbt.get_reference_mut(idx))
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.find_idx(key).is_some()
    }

    pub fn remove(&mut self, key: &str) -> bool {
        if let Some(idx) = self.get_self_node_mut().remove(key) {
            self.nbt.remove_node(idx);
            true
        } else {
            false
        }
    }
}

#[derive(Copy, Clone)]
pub struct ListRef<'a> {
    pub(crate) nbt: &'a NBT,
    pub(crate) node_idx: usize,
    pub(crate) children_type: TagType,
}

impl<'a> Debug for ListRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty::to_pretty_debug_list(f, self)
    }
}

impl PartialEq for ListRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        let (self_type, self_children) = self.get_self_node();
        let (other_type, other_children) = other.get_self_node();

        if self_type != other_type || self_children.len() != other_children.len() {
            return false;
        }

        let zipped = self_children.iter().zip(other_children.iter());
        for (self_child, other_child) in zipped {
            if self.nbt.get_reference(*self_child) != other.nbt.get_reference(*other_child) {
                return false;
            }
        }

        true
    }
}

impl<'a> ListRef<'a> {
    fn clone_into(&self, mut into: ListRefMut<'_>) {
        for child in self.iter() {
            match child {
                NBTRef::Byte(v) => into.insert_byte(*v),
                NBTRef::Short(v) => into.insert_short(*v),
                NBTRef::Int(v) => into.insert_int(*v),
                NBTRef::Long(v) => into.insert_long(*v),
                NBTRef::Float(v) => into.insert_float(*v),
                NBTRef::Double(v) => into.insert_double(*v),
                NBTRef::ByteArray(v) => into.insert_byte_array(v.clone()),
                NBTRef::String(v) => into.insert_string(v.clone()),
                NBTRef::List(v) => v.clone_into(into.create_list(v.children_type)),
                NBTRef::Compound(v) => v.clone_into(into.create_compound()),
                NBTRef::IntArray(v) => into.insert_int_array(v.clone()),
                NBTRef::LongArray(v) => into.insert_long_array(v.clone()),
            }
        }
    }

    pub(crate) fn get_self_node(&self) -> (TagType, &Vec<usize>) {
        match self.nbt.nodes.get(self.node_idx) {
            Some(NBTNode::List { type_id, children }) => (*type_id, children),
            _ => unsafe { unreachable_unchecked() },
        }
    }

    pub fn len(&self) -> usize {
        self.get_self_node().1.len()
    }

    pub fn get(&self, index: usize) -> Option<NBTRef<'_>> {
        let (_, children) = self.get_self_node();
        let idx = children.get(index)?;
        Some(self.nbt.get_reference(*idx))
    }

    super::enumerate_basic_types!(super::get_list);

    pub fn get_numeric<T: num::FromPrimitive>(&self, index: usize) -> Option<T> {
        let (_, children) = self.get_self_node();
        let idx = children.get(index)?;
        match &self.nbt.nodes[*idx] {
            NBTNode::Byte(v) => T::from_i8(*v),
            NBTNode::Short(v) => T::from_i16(*v),
            NBTNode::Int(v) => T::from_i32(*v),
            NBTNode::Long(v) => T::from_i64(*v),
            NBTNode::Float(v) => T::from_f32(*v),
            NBTNode::Double(v) => T::from_f64(*v),
            NBTNode::ByteArray(_) => None,
            NBTNode::String(_) => None,
            NBTNode::List {
                type_id: _,
                children: _,
            } => None,
            NBTNode::Compound(_) => None,
            NBTNode::IntArray(_) => None,
            NBTNode::LongArray(_) => None,
        }
    }

    pub fn iter(&self) -> ListIterator<'_> {
        ListIterator {
            nbt: self.nbt,
            indices: self.get_self_node().1,
            index: 0,
        }
    }
}

pub struct ListRefMut<'a> {
    pub(crate) nbt: &'a mut NBT,
    pub(crate) node_idx: usize,
}

impl<'a> Debug for ListRefMut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty::to_pretty_debug_list_mut(f, self)
    }
}

impl<'a> ListRefMut<'a> {
    pub(crate) fn get_self_node(&self) -> (TagType, &Vec<usize>) {
        match self.nbt.nodes.get(self.node_idx) {
            Some(NBTNode::List { type_id, children }) => (*type_id, children),
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn get_self_node_mut(&mut self) -> (TagType, &mut Vec<usize>) {
        match self.nbt.nodes.get_mut(self.node_idx) {
            Some(NBTNode::List { type_id, children }) => (*type_id, children),
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn insert_node(&mut self, node: NBTNode) -> usize {
        let (type_id, _) = self.get_self_node_mut();
        if type_id != node.get_type() {
            panic!("Tried to insert {:?} into a list of {:?}", node.get_type(), type_id);
        }

        let idx = self.nbt.nodes.insert(node);
        self.get_self_node_mut().1.push(idx);
        idx
    }

    fn set_node_at(&mut self, index: usize, node: NBTNode) -> usize {
        let (type_id, children) = self.get_self_node_mut();
        if type_id != node.get_type() {
            panic!("Tried to insert {:?} into a list of {:?}", node.get_type(), type_id);
        }

        if index == children.len() {
            return self.insert_node(node);
        }

        let idx = *children.get(index).unwrap();
        let _ = std::mem::replace(&mut self.nbt.nodes[idx], node);
        idx
    }

    pub fn len(&self) -> usize {
        self.get_self_node().1.len()
    }

    pub fn move_index(&mut self, from: usize, to: usize) -> bool {
        let (_, children) = self.get_self_node_mut();
        if from >= children.len() || to >= children.len() || from == to {
            return false;
        }

        let entry = children.remove(from);
        let mut insert_at = to;
        if insert_at > children.len() {
            insert_at = children.len();
        }
        children.insert(insert_at, entry);
        true
    }

    pub fn get(&self, index: usize) -> Option<NBTRef<'_>> {
        let (_, children) = self.get_self_node();
        let idx = children.get(index)?;
        Some(self.nbt.get_reference(*idx))
    }

    super::enumerate_basic_types!(super::get_list);

    pub fn get_numeric<T: num::FromPrimitive>(&self, index: usize) -> Option<T> {
        let (_, children) = self.get_self_node();
        let idx = children.get(index)?;
        match &self.nbt.nodes[*idx] {
            NBTNode::Byte(v) => T::from_i8(*v),
            NBTNode::Short(v) => T::from_i16(*v),
            NBTNode::Int(v) => T::from_i32(*v),
            NBTNode::Long(v) => T::from_i64(*v),
            NBTNode::Float(v) => T::from_f32(*v),
            NBTNode::Double(v) => T::from_f64(*v),
            NBTNode::ByteArray(_) => None,
            NBTNode::String(_) => None,
            NBTNode::List {
                type_id: _,
                children: _,
            } => None,
            NBTNode::Compound(_) => None,
            NBTNode::IntArray(_) => None,
            NBTNode::LongArray(_) => None,
        }
    }

    super::enumerate_basic_types!(super::insert_list);
    super::enumerate_basic_types!(super::set_list_at);

    pub fn create_compound(&mut self) -> CompoundRefMut<'_> {
        let idx = self.insert_node(NBTNode::Compound(Default::default()));

        CompoundRefMut {
            nbt: self.nbt,
            node_idx: idx,
        }
    }

    pub fn create_list(&mut self, type_id: TagType) -> ListRefMut<'_> {
        let idx = self.insert_node(NBTNode::List {
            type_id,
            children: Default::default(),
        });

        ListRefMut {
            nbt: self.nbt,
            node_idx: idx,
        }
    }
}

pub struct ListIterator<'a> {
    nbt: &'a NBT,
    indices: &'a [usize],
    index: usize,
}

impl<'a> Iterator for ListIterator<'a> {
    type Item = NBTRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.indices.len() {
            None
        } else {
            let next = self.nbt.get_reference(self.indices[self.index]);
            self.index += 1;
            Some(next)
        }
    }
}

pub struct CompoundIterator<'a> {
    nbt: &'a NBT,
    compound: &'a NBTCompound,
    index: usize,
}

impl<'a> Iterator for CompoundIterator<'a> {
    type Item = (&'a str, NBTRef<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.compound.0.len() {
            None
        } else {
            let entry = &self.compound.0[self.index];
            let next = self.nbt.get_reference(entry.1);
            self.index += 1;
            Some((&entry.0, next))
        }
    }
}
