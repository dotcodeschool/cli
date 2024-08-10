use crate::monitor::StateMachine;

use self::v2::ListerV2;

pub mod v2;

pub enum ListerVersion {
    V2(ListerV2),
}

impl StateMachine for ListerVersion {
    fn run(self) -> Self {
        match self {
            ListerVersion::V2(lister) => ListerVersion::V2(lister.run()),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ListerVersion::V2(lister) => lister.is_finished(),
        }
    }
}
