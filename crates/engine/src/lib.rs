//! mofoim 的引擎。
//!
//! 用法：前端把資料檔的位元組交給 [`Model::from_bytes`]，建立 [`Engine`]，
//! 之後每次按鍵呼叫 [`Engine::handle_key`]，依回傳的 [`Snapshot`] 更新畫面。

mod engine;
pub mod lattice;
pub mod layout;
pub mod model;
pub mod syllable;

pub use engine::{Candidate, Engine, Key, Mode, Snapshot, PAGE_SIZE};
pub use lattice::Config;
pub use layout::Layout;
pub use model::{Model, ModelBuilder};
pub use syllable::Syllable;
