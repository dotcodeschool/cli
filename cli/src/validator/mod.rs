use crate::monitor::StateMachine;

use self::v1::ValidatorV1;

pub mod v1;

pub enum ValidatorVersion {
    V1(ValidatorV1),
}

impl StateMachine for ValidatorVersion {
    fn run(self) -> Self {
        match self {
            Self::V1(validator) => Self::V1(validator.run()),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ValidatorVersion::V1(validator) => validator.is_finished(),
        }
    }
}
