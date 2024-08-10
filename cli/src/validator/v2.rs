use colored::Colorize;
use derive_more::Constructor;
use indicatif::ProgressBar;

use crate::{db::hash, parsing::v2::JsonCourseV2, str_res::DOTCODESCHOOL};

use super::Validator;

#[derive(PartialEq, Eq, Debug)]
pub enum ValidatorStateV2 {
    Loaded,
    Course,
    Stage {
        index_stage: usize,
    },
    Lesson {
        index_stage: usize,
        index_lesson: usize,
    },
    Suite {
        index_stage: usize,
        index_lesson: usize,
        index_suite: usize,
    },
    Test {
        index_stage: usize,
        index_lesson: usize,
        index_suite: usize,
        index_test: usize,
    },
    Fail {
        reason: String,
    },
    Pass,
    Finish,
}

#[derive(Constructor, Debug)]
pub struct ValidatorV2 {
    progress: ProgressBar,
    state: ValidatorStateV2,
    course: JsonCourseV2,
}

impl Validator for ValidatorV2 {
    fn run(self) -> Self {
        let Self { progress, state, course } = self;

        match state {
            ValidatorStateV2::Loaded => {
                progress.println("\n🔍 Validating format");

                Self { progress, state: ValidatorStateV2::Course, course }
            }
            ValidatorStateV2::Course => {
                let slug_expected = format!("0x{}", hash(&[&course.name]));
                if slug_expected != course.slug {
                    progress.println(format!(
                        "\n{}: {} ❌",
                        course.name.red().bold(),
                        course.slug.white()
                    ));

                    Self {
                        progress,
                        state: ValidatorStateV2::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                course.slug, slug_expected
                            ),
                        },
                        course,
                    }
                } else {
                    progress.println(format!(
                        "\n{}: {} ✅",
                        course.name.green().bold(),
                        course.slug.white()
                    ));

                    progress.inc(1);

                    Self {
                        progress,
                        state: ValidatorStateV2::Stage { index_stage: 0 },
                        course,
                    }
                }
            }
            ValidatorStateV2::Stage { index_stage } => {
                let stage = &course.stages[index_stage];

                let slug_expected =
                    format!("0x{}", hash(&[&course.name, &stage.name,]));
                if slug_expected != stage.slug {
                    progress.println(format!(
                        "╰─{}: {} ❌",
                        stage.name.red().bold(),
                        stage.slug.white()
                    ));

                    Self {
                        progress,
                        state: ValidatorStateV2::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                stage.slug, slug_expected
                            ),
                        },
                        course,
                    }
                } else {
                    progress.println(format!(
                        "╰─{}: {} ✅",
                        stage.name.green().bold(),
                        stage.slug.white()
                    ));

                    progress.inc(1);

                    Self {
                        progress,
                        state: ValidatorStateV2::Lesson {
                            index_stage,
                            index_lesson: 0,
                        },
                        course,
                    }
                }
            }
            ValidatorStateV2::Lesson { index_stage, index_lesson } => {
                let stage = &course.stages[index_stage];
                let lesson = &stage.lessons[index_lesson];

                let slug_expected = format!(
                    "0x{}",
                    hash(&[&course.name, &stage.name, &lesson.name,])
                );
                if slug_expected != lesson.slug {
                    progress.println(format!(
                        "  ╰─{}: {} ❌",
                        lesson.name.red().bold(),
                        lesson.slug.white()
                    ));

                    Self {
                        progress,
                        state: ValidatorStateV2::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                lesson.slug, slug_expected
                            ),
                        },
                        course,
                    }
                } else {
                    progress.println(format!(
                        "  ╰─{}: {} ✅",
                        lesson.name.green().bold(),
                        lesson.slug.white()
                    ));

                    progress.inc(1);

                    if lesson.suites.is_some() {
                        Self {
                            progress,
                            state: ValidatorStateV2::Suite {
                                index_stage,
                                index_lesson,
                                index_suite: 0,
                            },
                            course,
                        }
                    } else {
                        match (
                            index_stage + 1 < course.stages.len(),
                            index_lesson + 1 < stage.lessons.len(),
                        ) {
                            (_, true) => Self {
                                progress,
                                state: ValidatorStateV2::Lesson {
                                    index_stage,
                                    index_lesson: index_lesson + 1,
                                },
                                course,
                            },
                            (true, false) => Self {
                                progress,
                                state: ValidatorStateV2::Stage {
                                    index_stage: index_stage + 1,
                                },
                                course,
                            },
                            (false, false) => Self {
                                progress,
                                state: ValidatorStateV2::Pass,
                                course,
                            },
                        }
                    }
                }
            }
            ValidatorStateV2::Suite {
                index_stage,
                index_lesson,
                index_suite,
            } => {
                let stage = &course.stages[index_stage];
                let lesson = &stage.lessons[index_lesson];
                let suite = &lesson.suites.as_ref().expect(
                    "Suite has been checked to be Some in previous state",
                )[index_suite];

                let slug_expected = format!(
                    "0x{}",
                    hash(&[
                        &course.name,
                        &stage.name,
                        &lesson.name,
                        &suite.name,
                    ])
                );
                if slug_expected != suite.slug {
                    progress.println(format!(
                        "    ╰─{}: {} ❌",
                        suite.name.red().bold(),
                        suite.slug.white()
                    ));

                    Self {
                        progress,
                        state: ValidatorStateV2::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                suite.slug, slug_expected
                            ),
                        },
                        course,
                    }
                } else {
                    progress.println(format!(
                        "    ╰─{}: {} ✅",
                        suite.name.green().bold(),
                        suite.slug.white()
                    ));

                    progress.inc(1);

                    Self {
                        progress,
                        state: ValidatorStateV2::Test {
                            index_stage,
                            index_lesson,
                            index_suite,
                            index_test: 0,
                        },
                        course,
                    }
                }
            }
            ValidatorStateV2::Test {
                index_stage,
                index_lesson,
                index_suite,
                index_test,
            } => {
                let stage = &course.stages[index_stage];
                let lesson = &stage.lessons[index_lesson];
                let suites = &lesson.suites.as_ref().expect(
                    "Suite has been checked to be Some in previous state",
                );
                let suite = &suites[index_suite];
                let test = &suite.tests[index_test];

                let slug_expected = format!(
                    "0x{}",
                    hash(&[
                        &course.name,
                        &stage.name,
                        &lesson.name,
                        &suite.name,
                        &test.name,
                    ])
                );
                if slug_expected != test.slug {
                    progress.println(format!(
                        "      ╰─{}: {} ❌",
                        test.name.red().bold(),
                        test.slug.white()
                    ));

                    Self {
                        progress,
                        state: ValidatorStateV2::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                test.slug, slug_expected
                            ),
                        },
                        course,
                    }
                } else {
                    progress.println(format!(
                        "      ╰─{}: {} ✅",
                        test.name.green().bold(),
                        test.slug.white()
                    ));

                    progress.inc(1);

                    match (
                        index_stage + 1 < course.stages.len(),
                        index_lesson + 1 < stage.lessons.len(),
                        index_suite + 1 < suites.len(),
                        index_test + 1 < suite.tests.len(),
                    ) {
                        (_, _, _, true) => Self {
                            progress,
                            state: ValidatorStateV2::Test {
                                index_stage,
                                index_lesson,
                                index_suite,
                                index_test: index_test + 1,
                            },
                            course,
                        },
                        (_, _, true, false) => Self {
                            progress,
                            state: ValidatorStateV2::Suite {
                                index_stage,
                                index_lesson,
                                index_suite: index_suite + 1,
                            },
                            course,
                        },
                        (_, true, false, false) => Self {
                            progress,
                            state: ValidatorStateV2::Lesson {
                                index_stage,
                                index_lesson: index_lesson + 1,
                            },
                            course,
                        },
                        (true, false, false, false) => Self {
                            progress,
                            state: ValidatorStateV2::Stage {
                                index_stage: index_stage + 1,
                            },
                            course,
                        },
                        (false, false, false, false) => Self {
                            progress,
                            state: ValidatorStateV2::Pass,
                            course,
                        },
                    }
                }
            }
            ValidatorStateV2::Fail { reason } => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", reason.red().bold()));

                Self { progress, state: ValidatorStateV2::Finish, course }
            }
            ValidatorStateV2::Pass => {
                progress.finish_and_clear();
                progress.println(
                    "\n🏁 Course format is valid".green().bold().to_string(),
                );

                Self { progress, state: ValidatorStateV2::Finish, course }
            }
            ValidatorStateV2::Finish => {
                Self { progress, state: ValidatorStateV2::Finish, course }
            }
        }
    }

    fn is_finished(&self) -> bool {
        self.state == ValidatorStateV2::Finish
    }
}
