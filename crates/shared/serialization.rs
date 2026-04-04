//! Serialization utilities using MessagePack.

use crate::error::{Result, SnappixError};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Serialize a value to MessagePack bytes.
pub fn to_msgpack<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    rmp_serde::to_vec(value).map_err(SnappixError::from)
}

/// Deserialize a value from MessagePack bytes.
pub fn from_msgpack<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    rmp_serde::from_slice(bytes).map_err(SnappixError::from)
}

/// Serialize a value to MessagePack and write to a writer.
pub fn to_msgpack_writer<T: Serialize, W: std::io::Write>(value: &T, writer: &mut W) -> Result<()> {
    rmp_serde::encode::write(writer, value).map_err(SnappixError::from)
}

/// Deserialize a value from a MessagePack reader.
pub fn from_msgpack_reader<T: DeserializeOwned, R: std::io::Read>(reader: &mut R) -> Result<T> {
    rmp_serde::decode::from_read(reader).map_err(SnappixError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestStruct {
        name: String,
        value: i32,
    }

    #[test]
    fn test_msgpack_roundtrip() {
        let original = TestStruct {
            name: "test".to_string(),
            value: 42,
        };

        let bytes = to_msgpack(&original).unwrap();
        let decoded: TestStruct = from_msgpack(&bytes).unwrap();

        assert_eq!(original, decoded);
    }
}
