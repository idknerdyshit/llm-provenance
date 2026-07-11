//! Fail-closed serialization into the I-JSON number subset used by RFC 8785.

use serde::Serialize;
use serde::ser::{
    Error as _, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

use crate::{Error, Result};

const MAX_SAFE_INTEGER: i128 = 9_007_199_254_740_991;
const NUMBER_ERROR_PREFIX: &str = "llm-provenance invalid I-JSON number: ";

pub(crate) fn to_i_json_value<T: Serialize + ?Sized>(value: &T) -> Result<serde_json::Value> {
    serde_json::to_value(Validated(value)).map_err(|error| {
        let message = error.to_string();
        if let Some(reason) = message.strip_prefix(NUMBER_ERROR_PREFIX) {
            Error::InvalidIJsonNumber(reason.to_owned())
        } else {
            Error::Serialization(message)
        }
    })
}

struct Validated<'a, T: ?Sized>(&'a T);

impl<T: Serialize + ?Sized> Serialize for Validated<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.serialize(ValidatingSerializer(serializer))
    }
}

struct ValidatingSerializer<S>(S);

impl<S: Serializer> ValidatingSerializer<S> {
    fn signed(value: i128) -> std::result::Result<(), S::Error> {
        if (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
            Ok(())
        } else {
            Err(S::Error::custom(format!(
                "{NUMBER_ERROR_PREFIX}integer {value} exceeds the interoperable range ±(2^53-1)"
            )))
        }
    }

    fn unsigned(value: u128) -> std::result::Result<(), S::Error> {
        if value <= MAX_SAFE_INTEGER as u128 {
            Ok(())
        } else {
            Err(S::Error::custom(format!(
                "{NUMBER_ERROR_PREFIX}integer {value} exceeds the interoperable range ±(2^53-1)"
            )))
        }
    }

    fn float(value: f64) -> std::result::Result<(), S::Error> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(S::Error::custom(format!(
                "{NUMBER_ERROR_PREFIX}NaN and infinite floating-point values are not valid I-JSON"
            )))
        }
    }
}

impl<S: Serializer> Serializer for ValidatingSerializer<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = ValidatingCompound<S::SerializeSeq>;
    type SerializeTuple = ValidatingCompound<S::SerializeTuple>;
    type SerializeTupleStruct = ValidatingCompound<S::SerializeTupleStruct>;
    type SerializeTupleVariant = ValidatingCompound<S::SerializeTupleVariant>;
    type SerializeMap = ValidatingCompound<S::SerializeMap>;
    type SerializeStruct = ValidatingCompound<S::SerializeStruct>;
    type SerializeStructVariant = ValidatingCompound<S::SerializeStructVariant>;

    fn serialize_bool(self, value: bool) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_bool(value)
    }
    fn serialize_i8(self, value: i8) -> std::result::Result<Self::Ok, Self::Error> {
        Self::signed(value.into())?;
        self.0.serialize_i8(value)
    }
    fn serialize_i16(self, value: i16) -> std::result::Result<Self::Ok, Self::Error> {
        Self::signed(value.into())?;
        self.0.serialize_i16(value)
    }
    fn serialize_i32(self, value: i32) -> std::result::Result<Self::Ok, Self::Error> {
        Self::signed(value.into())?;
        self.0.serialize_i32(value)
    }
    fn serialize_i64(self, value: i64) -> std::result::Result<Self::Ok, Self::Error> {
        Self::signed(value.into())?;
        self.0.serialize_i64(value)
    }
    fn serialize_i128(self, value: i128) -> std::result::Result<Self::Ok, Self::Error> {
        Self::signed(value)?;
        self.0.serialize_i128(value)
    }
    fn serialize_u8(self, value: u8) -> std::result::Result<Self::Ok, Self::Error> {
        Self::unsigned(value.into())?;
        self.0.serialize_u8(value)
    }
    fn serialize_u16(self, value: u16) -> std::result::Result<Self::Ok, Self::Error> {
        Self::unsigned(value.into())?;
        self.0.serialize_u16(value)
    }
    fn serialize_u32(self, value: u32) -> std::result::Result<Self::Ok, Self::Error> {
        Self::unsigned(value.into())?;
        self.0.serialize_u32(value)
    }
    fn serialize_u64(self, value: u64) -> std::result::Result<Self::Ok, Self::Error> {
        Self::unsigned(value.into())?;
        self.0.serialize_u64(value)
    }
    fn serialize_u128(self, value: u128) -> std::result::Result<Self::Ok, Self::Error> {
        Self::unsigned(value)?;
        self.0.serialize_u128(value)
    }
    fn serialize_f32(self, value: f32) -> std::result::Result<Self::Ok, Self::Error> {
        Self::float(value.into())?;
        self.0.serialize_f32(value)
    }
    fn serialize_f64(self, value: f64) -> std::result::Result<Self::Ok, Self::Error> {
        Self::float(value)?;
        self.0.serialize_f64(value)
    }
    fn serialize_char(self, value: char) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_char(value)
    }
    fn serialize_str(self, value: &str) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_str(value)
    }
    fn serialize_bytes(self, value: &[u8]) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_bytes(value)
    }
    fn serialize_none(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }
    fn serialize_some<T: Serialize + ?Sized>(
        self,
        value: &T,
    ) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_some(&Validated(value))
    }
    fn serialize_unit(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }
    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(name)
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(name, variant_index, variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_struct(name, &Validated(value))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> std::result::Result<Self::Ok, Self::Error> {
        self.0
            .serialize_newtype_variant(name, variant_index, variant, &Validated(value))
    }
    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> std::result::Result<Self::SerializeSeq, Self::Error> {
        self.0.serialize_seq(len).map(ValidatingCompound)
    }
    fn serialize_tuple(self, len: usize) -> std::result::Result<Self::SerializeTuple, Self::Error> {
        self.0.serialize_tuple(len).map(ValidatingCompound)
    }
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> std::result::Result<Self::SerializeTupleStruct, Self::Error> {
        self.0
            .serialize_tuple_struct(name, len)
            .map(ValidatingCompound)
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> std::result::Result<Self::SerializeTupleVariant, Self::Error> {
        self.0
            .serialize_tuple_variant(name, variant_index, variant, len)
            .map(ValidatingCompound)
    }
    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> std::result::Result<Self::SerializeMap, Self::Error> {
        self.0.serialize_map(len).map(ValidatingCompound)
    }
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> std::result::Result<Self::SerializeStruct, Self::Error> {
        self.0.serialize_struct(name, len).map(ValidatingCompound)
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> std::result::Result<Self::SerializeStructVariant, Self::Error> {
        self.0
            .serialize_struct_variant(name, variant_index, variant, len)
            .map(ValidatingCompound)
    }
    fn collect_str<T: std::fmt::Display + ?Sized>(
        self,
        value: &T,
    ) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.collect_str(value)
    }
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

struct ValidatingCompound<C>(C);

impl<C: SerializeSeq> SerializeSeq for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_element(&Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<C: SerializeTuple> SerializeTuple for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_element(&Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<C: SerializeTupleStruct> SerializeTupleStruct for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_field(&Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<C: SerializeTupleVariant> SerializeTupleVariant for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_field(&Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<C: SerializeMap> SerializeMap for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_key<T: Serialize + ?Sized>(
        &mut self,
        key: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_key(&Validated(key))
    }
    fn serialize_value<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_value(&Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<C: SerializeStruct> SerializeStruct for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_field(key, &Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<C: SerializeStructVariant> SerializeStructVariant for ValidatingCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        self.0.serialize_field(key, &Validated(value))
    }
    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}
