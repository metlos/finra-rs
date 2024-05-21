//! This is a simple wrapper around the FINRA REST API.
//!
//! Almost no features are currently implemented, only fetching the consolidated short interest.
//!
//! The basic filtering and limiting of the returned data is implemented though.
//!
//! The `tokio` feature makes the library use the tokio-specific replacements of the standard
//! library's synchronization primitives but has no other functional differences.

mod error;
mod finra;
mod pager;
mod query;
pub use error::*;
pub use finra::*;
pub use query::*;
