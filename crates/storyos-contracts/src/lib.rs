//! StoryOS-owned source for public contract shapes and deterministic generated artifacts.

mod digest;
mod release1;
mod release1_artifacts;
mod stage1_crosswalk;
mod stage1_delivery;
mod stage1_selection;

pub use release1::{
    GET_PROTOCOL_PROFILE_METHOD, GET_PROTOCOL_PROFILE_PATH, Release1CompatibilityIdentity,
    Release1ProtocolProfile,
};
pub use release1_artifacts::{
    check_release1_artifacts, release1_protocol_profile, write_release1_artifacts,
};
pub use stage1_crosswalk::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, check_crosswalk, generate_crosswalk, repository_root,
    write_crosswalk,
};
