pub mod watcher;
pub mod classifier;
pub mod pipeline;
pub mod chunk_adaptive;
pub mod metadata_transformer;

pub use watcher::{FileWatcher, FileEvent};
pub use classifier::{PathClassifier, ClassificationResult};
pub use pipeline::{IndexingPipeline, IndexingCoordinator};
