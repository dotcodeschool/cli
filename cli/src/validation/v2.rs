use crate::parsing::v2::JsonCourseV2;

pub enum ValidatorStateV2 {
    Course,
    Stage { index_stage: usize },
    Lesson { index_stage: usize, index_lesson: usize },
    Suite { index_stage: usize, index_lesson: usize, index_suite: usize },
    Test { index_stage: usize, index_lesson: usize, index_suite: usize },
}

pub struct ValidatorV2 {
    course: JsonCourseV2,
    state: ValidatorStateV2,
}
