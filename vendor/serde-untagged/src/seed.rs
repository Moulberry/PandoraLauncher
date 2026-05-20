use crate::any::ErasedValue;
use alloc::boxed::Box;
use serde::de::{DeserializeSeed, Deserializer};

pub(crate) trait ErasedDeserializeSeed<'de> {
    fn erased_deserialize(
        &mut self,
        deserializer: Box<dyn erased_serde::Deserializer<'de> + '_>,
    ) -> Result<ErasedValue, erased_serde::Error>;
}

impl<'de, Seed> ErasedDeserializeSeed<'de> for Option<Seed>
where
    Seed: DeserializeSeed<'de>,
{
    fn erased_deserialize(
        &mut self,
        deserializer: Box<dyn erased_serde::Deserializer<'de> + '_>,
    ) -> Result<ErasedValue, erased_serde::Error> {
        self.take()
            .unwrap()
            .deserialize(deserializer)
            .map(|value| unsafe { ErasedValue::new::<Seed::Value>(value) })
    }
}

impl<'de> DeserializeSeed<'de> for &mut dyn ErasedDeserializeSeed<'de> {
    type Value = ErasedValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let deserializer = Box::new(<dyn erased_serde::Deserializer>::erase(deserializer));
        self.erased_deserialize(deserializer)
            .map_err(serde::de::Error::custom)
    }
}
