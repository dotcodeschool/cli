use crate::monitor::StateMachine;

use self::v1::ListerV1;

pub mod v1;

pub enum ListerVersion {
    V1(ListerV1),
}

impl StateMachine for ListerVersion {
    fn run(self) -> Self {
        match self {
            ListerVersion::V1(lister) => ListerVersion::V1(lister.run()),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ListerVersion::V1(lister) => lister.is_finished(),
        }
    }
}
