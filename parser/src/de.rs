use std::ops::{AddAssign, MulAssign, Neg};

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::error::{Error, Result};
use std::str::FromStr;

pub struct Deserializer<'de> {
    input: &'de str,
}

impl<'de> Deserializer<'de> {
    pub fn from_str(input: &'de str) -> Self {
        Deserializer { input }
    }
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut de = Deserializer::from_str(s);
    let mut deserializer = TopDeserializer::new(&mut de);
    let t = T::deserialize(&mut deserializer)?;
    if de.input.is_empty() {
        Ok(t)
    } else {
        println!("{}", &de.input);
        Err(Error::TrailingCharacters)
    }
}

impl<'de> Deserializer<'de> {
    fn parse_line(&mut self) -> Result<&'de str> {
        match self.input.find("\n") {
            Some(len) => {
                let s = &self.input[..len];
                self.input = &self.input[len + 1..];
                Ok(s)
            }
            None => {
                if self.input.len() == 0 {
                    Err(Error::Eof)
                } else {
                    let s = self.input;
                    self.input = "";
                    Ok(s)
                }
            }
        }
    }

    fn peek_delimiter(&mut self) -> Result<bool> {
        match self.input.find("\n") {
            Some(len) => Ok(len == 0),
            None => Ok(self.input.len() == 0),
        }
    }

    fn parse_field_name(&mut self) -> Result<&'de str> {
        let line = self.parse_line()?;
        if line.starts_with("%") && line.ends_with("%") {
            Ok(&line[1..line.len() - 1])
        } else {
            Err(Error::FieldNameUnexpectedWrapper(String::from(line)))
        }
    }

    fn parse_string(&mut self) -> Result<&'de str> {
        let line = self.parse_line()?;
        if line.is_empty() {
            Err(Error::DelimiterNotExpected)
        } else {
            Ok(line)
        }
    }

    fn parse_char(&mut self) -> Result<char> {
        let line = self.parse_line()?;
        if line.is_empty() {
            Err(Error::DelimiterNotExpected)
        } else if line.len() != 1 {
            Err(Error::CharOverflow)
        } else {
            Ok(line.chars().nth(0).unwrap())
        }
    }

    fn parse_delimiter(&mut self) -> Result<()> {
        match self.input.find("\n") {
            Some(len) => {
                let s = &self.input[..len];
                self.input = &self.input[len + 1..];
                if s.is_empty() {
                    Ok(())
                } else {
                    Err(Error::DelimiterExpected)
                }
            }
            None => Ok(()),
        }
    }

    fn parse_unsigned<T>(&mut self) -> Result<T>
    where
        T: AddAssign<T> + MulAssign<T> + FromStr,
    {
        let line = self.parse_line()?;
        match line.parse() {
            Ok(n) => Ok(n),
            Err(_) => Err(Error::IntegerError),
        }
    }

    fn parse_signed<T>(&mut self) -> Result<T>
    where
        T: Neg<Output = T> + AddAssign<T> + MulAssign<T> + FromStr,
    {
        let line = self.parse_line()?;
        match line.parse() {
            Ok(n) => Ok(n),
            Err(_) => Err(Error::IntegerError),
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, _: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_bool<V>(self, _: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse_signed()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_signed()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse_signed()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse_signed()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse_unsigned()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse_unsigned()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse_unsigned()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_unsigned()?)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_char(self.parse_char()?)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // The `Serializer` implementation on the previous page serialized byte
    // arrays as JSON arrays of bytes. Handle that representation here.
    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_delimiter()? {
            self.parse_delimiter()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_delimiter()? {
            self.parse_delimiter()?;
            visitor.visit_unit()
        } else {
            Err(Error::DelimiterExpected)
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let value = visitor.visit_seq(NewlineSeparated::new(&mut self))?;
        Ok(value)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let value = visitor.visit_map(NewlineSeparated::new(&mut self))?;
        Ok(value)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // _visitor.visit_enum(self.parse_string()?.into_deserializer())
        Err(Error::NotSupported)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.parse_field_name()?)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

struct TopDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> TopDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        TopDeserializer { de }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut TopDeserializer<'a, 'de> {
    type Error = Error;

    fn deserialize_any<V>(self, _: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_bool<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_i8<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_i16<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_i32<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_i64<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_u8<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_u16<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_u32<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_u64<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_str<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_string<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_seq<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_map(visitor)
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_struct(name, fields, visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::StructExpected)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }
}

struct ValueDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    allow_arrays: bool,
}

impl<'a, 'de> ValueDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, allow_arrays: bool) -> Self {
        ValueDeserializer { de, allow_arrays }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut ValueDeserializer<'a, 'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_any(visitor)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_bool(visitor)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_i8(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_i16(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_i32(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_i64(visitor)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_u8(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_u16(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_u32(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_u64(visitor)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_f32(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_f64(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_char(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_str(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_string(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_bytes(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_byte_buf(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_option(visitor)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_unit(visitor)
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_unit_struct(name, visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_newtype_struct(name, visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        if self.allow_arrays {
            self.de.deserialize_seq(visitor)
        } else {
            Err(Error::NotSupported)
        }
    }

    fn deserialize_tuple<V>(self, size: usize, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        if self.allow_arrays {
            self.de.deserialize_tuple(size, visitor)
        } else {
            Err(Error::NotSupported)
        }
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        size: usize,
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        if self.allow_arrays {
            self.de.deserialize_tuple_struct(name, size, visitor)
        } else {
            Err(Error::NotSupported)
        }
    }

    fn deserialize_map<V>(self, _: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_struct<V>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_enum<V>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_identifier<V>(self, _: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value>
    where
        V: Visitor<'de>,
    {
        self.de.deserialize_ignored_any(visitor)
    }
}

struct NewlineSeparated<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> NewlineSeparated<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        NewlineSeparated { de }
    }
}

impl<'de, 'a> SeqAccess<'de> for NewlineSeparated<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.de.peek_delimiter()? {
            return Ok(None);
        }
        let mut deserializer = ValueDeserializer::new(self.de, false);
        seed.deserialize(&mut deserializer).map(Some)
    }
}

impl<'de, 'a> MapAccess<'de> for NewlineSeparated<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        if self.de.peek_delimiter()? {
            return Ok(None);
        }
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let mut deserializer = ValueDeserializer::new(self.de, true);
        let result = seed.deserialize(&mut deserializer)?;
        self.de.parse_delimiter()?;
        Ok(result)
    }
}

// struct Enum<'a, 'de: 'a> {
//     de: &'a mut Deserializer<'de>,
// }
//
// impl<'a, 'de> Enum<'a, 'de> {
//     fn new(de: &'a mut Deserializer<'de>) -> Self {
//         Enum { de }
//     }
// }
//
// // `EnumAccess` is provided to the `Visitor` to give it the ability to determine
// // which variant of the enum is supposed to be deserialized.
// //
// // Note that all enum deserialization methods in Serde refer exclusively to the
// // "externally tagged" enum representation.
// impl<'de, 'a> EnumAccess<'de> for Enum<'a, 'de> {
//     type Error = Error;
//     type Variant = Self;
//
//     fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
//         where
//             V: DeserializeSeed<'de>,
//     {
//         // The `deserialize_enum` method parsed a `{` character so we are
//         // currently inside of a map. The seed will be deserializing itself from
//         // the key of the map.
//         let val = seed.deserialize(&mut *self.de)?;
//         // Parse the colon separating map key from value.
//         if self.de.next_char()? == ':' {
//             Ok((val, self))
//         } else {
//             Err(Error::ExpectedMapColon)
//         }
//     }
// }
//
// // `VariantAccess` is provided to the `Visitor` to give it the ability to see
// // the content of the single variant that it decided to deserialize.
// impl<'de, 'a> VariantAccess<'de> for Enum<'a, 'de> {
//     type Error = Error;
//
//     // If the `Visitor` expected this variant to be a unit variant, the input
//     // should have been the plain string case handled in `deserialize_enum`.
//     fn unit_variant(self) -> Result<()> {
//         Err(Error::ExpectedString)
//     }
//
//     // Newtype variants are represented in JSON as `{ NAME: VALUE }` so
//     // deserialize the value here.
//     fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
//         where
//             T: DeserializeSeed<'de>,
//     {
//         seed.deserialize(self.de)
//     }
//
//     // Tuple variants are represented in JSON as `{ NAME: [DATA...] }` so
//     // deserialize the sequence of data here.
//     fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
//         where
//             V: Visitor<'de>,
//     {
//         de::Deserializer::deserialize_seq(self.de, visitor)
//     }
//
//     // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }` so
//     // deserialize the inner map here.
//     fn struct_variant<V>(
//         self,
//         _fields: &'static [&'static str],
//         visitor: V,
//     ) -> Result<V::Value>
//         where
//             V: Visitor<'de>,
//     {
//         de::Deserializer::deserialize_map(self.de, visitor)
//     }
// }

#[cfg(test)]
mod test {
    use serde::Deserialize;

    #[test]
    fn test_struct() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Test {
            #[serde(rename = "NAME")]
            name: String,
            #[serde(rename = "DEPENDS")]
            depends: Vec<String>,
            #[serde(rename = "BUILDDATE")]
            build_date: u32,
        }

        let j = r#"%NAME%
mingw-w64-x86_64-vcdimager

%BUILDDATE%
1592300880

%DEPENDS%
mingw-w64-x86_64-libcdio
mingw-w64-x86_64-libxml2
mingw-w64-x86_64-popt"#;
        let expected = Test {
            build_date: 1592300880,
            depends: vec![
                "mingw-w64-x86_64-libcdio".to_owned(),
                "mingw-w64-x86_64-libxml2".to_owned(),
                "mingw-w64-x86_64-popt".to_owned(),
            ],
            name: "mingw-w64-x86_64-vcdimager".to_owned(),
        };
        assert_eq!(expected, crate::from_str(j).unwrap());
    }
}
