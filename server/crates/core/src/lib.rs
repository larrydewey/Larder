//! Shared domain types and pure logic for Larder: recipe models, ingredient
//! parsing, unit conversion, quantity formatting, grocery aisle classification
//! and trial recipe bookkeeping.

pub mod aisle;
pub mod categorize;
pub mod food;
pub mod format;
pub mod model;
pub mod parse;
pub mod shopping;
pub mod text;
pub mod trial;
pub mod units;

pub use model::{Ingredient, Instruction, RecipeData};
pub use trial::{RecipeStatus, TrialState};
