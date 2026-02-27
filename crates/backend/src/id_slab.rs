use slab::Slab;

#[derive(Debug)]
pub struct IdSlab<T: GetId> {
    slab: Slab<T>,
}

impl<T: GetId> Default for IdSlab<T> {
    fn default() -> Self {
        Self { slab: Default::default() }
    }
}

pub trait Id: PartialEq {
    fn get_index(&self) -> usize;
}

pub trait GetId {
    type Id: Id;

    fn get_id(&self) -> Self::Id;
}

impl<T: GetId> IdSlab<T> {
    pub fn get(&self, id: T::Id) -> Option<&T> {
        let v = self.slab.get(id.get_index())?;
        if v.get_id() != id {
            return None;
        }
        return Some(v);
    }

    pub fn get_mut(&mut self, id: T::Id) -> Option<&mut T> {
        let v = self.slab.get_mut(id.get_index())?;
        if v.get_id() != id {
            return None;
        }
        return Some(v);
    }

    pub fn remove(&mut self, id: T::Id) -> Option<T> {
        let v = self.slab.get(id.get_index())?;
        if v.get_id() != id {
            return None;
        }
        return self.slab.try_remove(id.get_index());
    }

    pub fn drain(&mut self) -> impl Iterator<Item = T> {
        self.slab.drain()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slab.iter().map(|(_, v)| v)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slab.iter_mut().map(|(_, v)| v)
    }

    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.slab.retain(|_, t| (f)(t));
    }

    pub fn insert(&mut self, f: impl FnOnce(usize) -> T) -> &mut T {
        let vacant = self.slab.vacant_entry();
        let v = (f)(vacant.key());
        assert_eq!(v.get_id().get_index(), vacant.key());
        vacant.insert(v)
    }
}
