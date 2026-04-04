//! ID generation utilities.

use uuid::Uuid;

/// Generate a new unique ID.
pub fn generate_id() -> Uuid {
    Uuid::new_v4()
}

/// Generate a new unique ID as a string.
pub fn generate_id_string() -> String {
    Uuid::new_v4().to_string()
}
