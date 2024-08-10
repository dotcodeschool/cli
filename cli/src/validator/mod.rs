use crate::monitor::StateMachine;

use self::v2::ValidatorV2;

pub mod v2;

pub enum ValidatorVersion {
    V2(ValidatorV2),
    Undefined,
}

impl StateMachine for ValidatorVersion {
    fn run(self) -> Self {
        match self {
            Self::V2(validator) => Self::V2(validator.run()),
            Self::Undefined => Self::Undefined,
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ValidatorVersion::V2(validator) => validator.is_finished(),
            ValidatorVersion::Undefined => true,
        }
    }
}
