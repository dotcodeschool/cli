use indicatif::ProgressBar;

use crate::parsing::{load_course, JsonCourseVersion, ParsingError};

use self::v2::{ValidatorStateV2, ValidatorV2};

pub mod v2;

pub enum ValidatorVersion {
    V2(ValidatorV2),
    Undefined,
}

pub trait Validator {
    fn run(self) -> Self;

    fn is_finished(&self) -> bool;
}

impl Validator for ValidatorVersion {
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
