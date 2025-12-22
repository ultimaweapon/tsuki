use crate::{Lua, Value};
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Formatter;
use serde::Deserializer;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Unexpected, Visitor};

/// Implementation of [Visitor] to deserialize any Lua value.
///
/// It is not safe to deserialize to [UnsafeValue] since the deserializer might contains [Lua]
/// instance, which can trigger GC.
pub struct ValueVisitor<'a, A>(&'a Lua<A>);

impl<'a, A> ValueVisitor<'a, A> {
    #[inline(always)]
    pub fn new(lua: &'a Lua<A>) -> Self {
        Self(lua)
    }
}

impl<'a, 'de, A> Visitor<'de> for ValueVisitor<'a, A> {
    type Value = Value<'a, A>;

    fn expecting(&self, formatter: &mut Formatter) -> core::fmt::Result {
        formatter.write_str("any valid Lua value")
    }

    #[inline]
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(match v {
            true => Value::True,
            false => Value::False,
        })
    }

    #[inline]
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Int(v))
    }

    #[inline]
    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v.into())
    }

    #[inline]
    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v.into())
    }

    #[inline]
    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v.into())
    }

    #[inline]
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Float(v.into()))
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_str(v)))
    }

    #[inline]
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_str(v)))
    }

    #[inline]
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_bytes(v)))
    }

    #[inline]
    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_bytes(v)))
    }

    #[inline]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Nil)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    #[inline]
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Nil)
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let t = self.0.create_table();
        let mut i = 1i64;

        while let Some(v) = seq.next_element_seed(ValueSeed(self.0))? {
            // SAFETY: We pass the same Lua to ValueSeed.
            // SAFETY: i is i64, which mean it is not possible for error.
            unsafe { t.set_unchecked(i, v).unwrap_unchecked() };
            i += 1;
        }

        Ok(Value::Table(t))
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let t = self.0.create_table();

        while let Some(k) = map.next_key_seed(KeySeed(self.0))? {
            let v = map.next_value_seed(ValueSeed(self.0))?;

            // SAFETY: We pass the same Lua to KeySeed and ValueSeed.
            // SAFETY: KeySeed never return nil or NaN, which mean it is not possible for error.
            unsafe { t.set_unchecked(k, v).unwrap_unchecked() };
        }

        Ok(Value::Table(t))
    }
}

/// Implementation of [DeserializeSeed] to deserialize any Lua value.
struct ValueSeed<'a, A>(&'a Lua<A>);

impl<'a, 'de, A> DeserializeSeed<'de> for ValueSeed<'a, A> {
    type Value = Value<'a, A>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor(self.0))
    }
}

/// Implementation of [Visitor] to deserialize table key.
struct KeyVisitor<'a, A>(&'a Lua<A>);

impl<'a, 'de, A> Visitor<'de> for KeyVisitor<'a, A> {
    type Value = Value<'a, A>;

    fn expecting(&self, formatter: &mut Formatter) -> core::fmt::Result {
        formatter.write_str("any valid table key")
    }

    #[inline]
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(match v {
            true => Value::True,
            false => Value::False,
        })
    }

    #[inline]
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Int(v))
    }

    #[inline]
    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v.into())
    }

    #[inline]
    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v.into())
    }

    #[inline]
    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v.into())
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v.is_nan() {
            true => Err(serde::de::Error::invalid_value(Unexpected::Float(v), &self)),
            false => Ok(Value::Float(v.into())),
        }
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_str(v)))
    }

    #[inline]
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_str(v)))
    }

    #[inline]
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_bytes(v)))
    }

    #[inline]
    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Str(self.0.create_bytes(v)))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

/// Implementation of [DeserializeSeed] to deserialize table key.
struct KeySeed<'a, A>(&'a Lua<A>);

impl<'a, 'de, A> DeserializeSeed<'de> for KeySeed<'a, A> {
    type Value = Value<'a, A>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(KeyVisitor(self.0))
    }
}
