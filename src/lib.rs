//! PolyTorus - 4-Layer Modular Blockchain Platform
//!
//! This library provides a modular blockchain architecture with separate layers for:
//! - Execution: Transaction processing and rollups
//! - Settlement: Dispute resolution and finalization  
//! - Consensus: Block ordering and validation
//! - Data Availability: Data storage and distribution

// Re-export the modular layer crates
pub use execution;
pub use settlement;
pub use consensus;
pub use data_availability;
pub use traits;