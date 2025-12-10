//! Database persistence module for PipeWire state.

pub mod db;
pub mod db_channel;

#[cfg(test)]
pub mod db_channel_test;

// Re-export the Database struct for convenience
pub use db::Database;