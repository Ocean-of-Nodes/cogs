//! Incidence queries — both projections of the «edge incident to
//! vertex» relation.
//!
//! - [`neighborhood`] — node-side: which entities does `id` connect
//!   to?
//! - [`edging`] — edge-side: which edges touch `id`?

pub mod edging;
pub mod neighborhood;