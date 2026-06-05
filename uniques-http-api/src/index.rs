pub mod loader;
mod query;
pub mod uniques_index;

pub use loader::load_index;
pub use query::QueryError;
pub use uniques_index::{CardResolveError, UniquesIndex};
pub(crate) use query::{
    build_bitmap, card_v2_from_index, cards_from_indices, families_from_bitmap, page_cards_v2,
};
