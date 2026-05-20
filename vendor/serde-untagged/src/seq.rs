use crate::any::ErasedValue;
use crate::error::{self, Error};
use crate::seed::ErasedDeserializeSeed;
use alloc::boxed::Box;
use serde::de::{Deserialize, DeserializeSeed, SeqAccess};

trait ErasedSeqAccess<'de> {
    fn erased_next_element_seed(
        &mut self,
        seed: &mut dyn ErasedDeserializeSeed<'de>,
    ) -> Result<Option<ErasedValue>, Error>;

    fn erased_size_hint(&self) -> Option<usize>;
}

pub struct Seq<'access, 'de> {
    erased: Box<dyn ErasedSeqAccess<'de> + 'access>,
}

impl<'access, 'de> Seq<'access, 'de> {
    pub(crate) fn new<A>(seq: A) -> Self
    where
        A: SeqAccess<'de> + 'access,
    {
        Seq {
            erased: Box::new(seq),
        }
    }

    /// Shorthand for `T::deserialize(serde::de::value::SeqAccessDeserializer::new(self))`.
    pub fn deserialize<T>(self) -> Result<T, Error>
    where
        T: Deserialize<'de>,
    {
        T::deserialize(serde::de::value::SeqAccessDeserializer::new(self))
    }
}

impl<'access, 'de> SeqAccess<'de> for Seq<'access, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.erased
            .erased_next_element_seed(&mut Some(seed))
            .map(|erased_value| match erased_value {
                Some(value) => Some(unsafe { ErasedValue::take::<T::Value>(value) }),
                None => None,
            })
    }

    fn size_hint(&self) -> Option<usize> {
        self.erased.erased_size_hint()
    }
}

impl<'de, Access> ErasedSeqAccess<'de> for Access
where
    Access: SeqAccess<'de>,
{
    fn erased_next_element_seed(
        &mut self,
        seed: &mut dyn ErasedDeserializeSeed<'de>,
    ) -> Result<Option<ErasedValue>, Error> {
        self.next_element_seed(seed).map_err(error::erase)
    }

    fn erased_size_hint(&self) -> Option<usize> {
        self.size_hint()
    }
}
