//! Fail-closed validation of the I-JSON number subset used by RFC 8785.

use serde::Serialize;
use serde::ser::{
    SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

use crate::{Error, Result};

const MAX_SAFE_INTEGER: i128 = 9_007_199_254_740_991;

pub(crate) fn validate_i_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    value
        .serialize(NumberValidator)
        .map_err(|error| Error::InvalidIJsonNumber(error.to_string()))
}

#[derive(Debug)]
struct ValidationError(String);

impl std::fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ValidationError {}

impl serde::ser::Error for ValidationError {
    fn custom<T: std::fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

#[derive(Clone, Copy)]
struct NumberValidator;

impl NumberValidator {
    fn signed(value: i128) -> std::result::Result<(), ValidationError> {
        if (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
            Ok(())
        } else {
            Err(ValidationError(format!(
                "integer {value} exceeds the interoperable range ±(2^53-1)"
            )))
        }
    }

    fn unsigned(value: u128) -> std::result::Result<(), ValidationError> {
        if value <= MAX_SAFE_INTEGER as u128 {
            Ok(())
        } else {
            Err(ValidationError(format!(
                "integer {value} exceeds the interoperable range ±(2^53-1)"
            )))
        }
    }

    fn float(value: f64) -> std::result::Result<(), ValidationError> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(ValidationError(
                "NaN and infinite floating-point values are not valid I-JSON".to_owned(),
            ))
        }
    }
}

impl Serializer for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, _: bool) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i8(self, value: i8) -> std::result::Result<(), Self::Error> {
        Self::signed(value.into())
    }
    fn serialize_i16(self, value: i16) -> std::result::Result<(), Self::Error> {
        Self::signed(value.into())
    }
    fn serialize_i32(self, value: i32) -> std::result::Result<(), Self::Error> {
        Self::signed(value.into())
    }
    fn serialize_i64(self, value: i64) -> std::result::Result<(), Self::Error> {
        Self::signed(value.into())
    }
    fn serialize_i128(self, value: i128) -> std::result::Result<(), Self::Error> {
        Self::signed(value)
    }
    fn serialize_u8(self, value: u8) -> std::result::Result<(), Self::Error> {
        Self::unsigned(value.into())
    }
    fn serialize_u16(self, value: u16) -> std::result::Result<(), Self::Error> {
        Self::unsigned(value.into())
    }
    fn serialize_u32(self, value: u32) -> std::result::Result<(), Self::Error> {
        Self::unsigned(value.into())
    }
    fn serialize_u64(self, value: u64) -> std::result::Result<(), Self::Error> {
        Self::unsigned(value.into())
    }
    fn serialize_u128(self, value: u128) -> std::result::Result<(), Self::Error> {
        Self::unsigned(value)
    }
    fn serialize_f32(self, value: f32) -> std::result::Result<(), Self::Error> {
        Self::float(value.into())
    }
    fn serialize_f64(self, value: f64) -> std::result::Result<(), Self::Error> {
        Self::float(value)
    }
    fn serialize_char(self, _: char) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_str(self, _: &str) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_bytes(self, _: &[u8]) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_none(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(
        self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_seq(
        self,
        _: Option<usize>,
    ) -> std::result::Result<Self::SerializeSeq, Self::Error> {
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> std::result::Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> std::result::Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> std::result::Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(self)
    }
    fn serialize_map(
        self,
        _: Option<usize>,
    ) -> std::result::Result<Self::SerializeMap, Self::Error> {
        Ok(self)
    }
    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> std::result::Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> std::result::Result<Self::SerializeStructVariant, Self::Error> {
        Ok(self)
    }
    fn collect_str<T: std::fmt::Display + ?Sized>(
        self,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}

impl SerializeSeq for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeTuple for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeTupleStruct for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeTupleVariant for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeMap for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_key<T: Serialize + ?Sized>(
        &mut self,
        key: &T,
    ) -> std::result::Result<(), Self::Error> {
        key.serialize(*self)
    }
    fn serialize_value<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeStruct for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeStructVariant for NumberValidator {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        value.serialize(*self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}
