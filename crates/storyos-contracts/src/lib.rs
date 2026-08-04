//! StoryOS-owned source for the deterministic Stage 1 contract crosswalk.
//! Runtime DTOs and handlers remain outside this first stack layer.

mod stage1_crosswalk;
mod stage1_selection;

pub use stage1_crosswalk::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, check_crosswalk, generate_crosswalk, repository_root,
    write_crosswalk,
};
