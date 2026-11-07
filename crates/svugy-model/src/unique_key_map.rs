//! Deserialize JSON objects into ordered maps that reject a repeated key, the one duplicate-key rule every JSON shape shares.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// Collect map entries, rejecting a key already seen.
pub struct UniqueKeyMapVisitor<K, V> {
    entry_types: PhantomData<(K, V)>,
}

impl<K, V> UniqueKeyMapVisitor<K, V> {
    /// Read a JSON object into an ordered map, failing on a repeated key instead of keeping the last value.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord + fmt::Display,
        V: Deserialize<'de>,
    {
        let visitor = Self {
            entry_types: PhantomData,
        };
        deserializer.deserialize_map(visitor)
    }
}

impl<'de, K, V> Visitor<'de> for UniqueKeyMapVisitor<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Display,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with unique keys")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Self::Value, A::Error> {
        let mut map = BTreeMap::new();
        while let Some(key) = entries.next_key::<K>()? {
            let is_duplicate = map.contains_key(&key);
            if is_duplicate {
                let duplicate_key_message = format!("duplicate key `{key}`");
                let duplicate_key_error = de::Error::custom(duplicate_key_message);
                return Err(duplicate_key_error);
            }
            let value = entries.next_value()?;
            map.insert(key, value);
        }
        Ok(map)
    }
}
