use crate::parsing::{load_course, JsonCourseVersion, ParsingError};

use self::v2::ValidatorV2;

pub mod v2;

pub enum ValidatorVersion {
    V2(ValidatorV2),
    Undefined,
}

pub trait Validatior {
    fn run(self) -> Self;

    fn new(path: &str) -> ValidatorVersion {
        match load_course(path) {
            Ok(course_version) => match course_version {
                JsonCourseVersion::V1(_) => ValidatorVersion::Undefined,
                JsonCourseVersion::V2(_) => todo!(),
            },
            Err(e) => {
                let msg = match e {
                    ParsingError::CourseFmtError(msg) => msg,
                    ParsingError::FileOpenError(msg) => msg,
                };
                log::error!("{msg}");

                ValidatorVersion::Undefined
            }
        }
    }
}
