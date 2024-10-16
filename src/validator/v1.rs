use colored::Colorize;
use indicatif::ProgressBar;

use crate::{
    db::hash, models::TesterDefinition, monitor::StateMachine,
    parsing::v1::JsonCourseV1,
};

#[derive(PartialEq, Eq, Debug)]
pub enum ValidatorStateV1 {
    Loaded,
    Course,
    Section {
        index_section: usize,
    },
    Lesson {
        index_section: usize,
        index_lesson: usize,
    },
    Suite {
        index_section: usize,
        index_lesson: usize,
        index_suite: usize,
    },
    Test {
        index_section: usize,
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

#[derive(Debug)]
pub struct ValidatorV1 {
    progress: ProgressBar,
    state: ValidatorStateV1,
    course: JsonCourseV1,
    tester: TesterDefinition,
}

impl ValidatorV1 {
    pub fn new(
        progress: ProgressBar,
        state: ValidatorStateV1,
        course: JsonCourseV1,
        tester: TesterDefinition,
    ) -> Self {
        Self { progress, state, course, tester }
    }
}

impl StateMachine for ValidatorV1 {
    fn run(self) -> Self {
        let Self { progress, state, course, tester } = self;

        match state {
            ValidatorStateV1::Loaded => {
                progress.println("\n🔍 Validating format");

                Self {
                    progress,
                    state: ValidatorStateV1::Course,
                    course,
                    tester,
                }
            }
            ValidatorStateV1::Course => {
                progress.println(format!(
                    "\n{}: {} ✅",
                    course.name.green().bold(),
                    course.slug.white()
                ));

                progress.inc(1);

                Self {
                    progress,
                    state: ValidatorStateV1::Section { index_section: 0 },
                    course,
                    tester,
                }
            }
            ValidatorStateV1::Section { index_section } => {
                let section = &tester.sections[index_section];

                progress.println(format!(
                    "╰─{}: {} ✅",
                    section.name.green().bold(),
                    section.slug.white()
                ));

                progress.inc(1);

                Self {
                    progress,
                    state: ValidatorStateV1::Lesson {
                        index_section,
                        index_lesson: 0,
                    },
                    course,
                    tester,
                }
            }
            ValidatorStateV1::Lesson { index_section, index_lesson } => {
                let section = &tester.sections[index_section];
                let lesson = &section.lessons[index_lesson];

                let slug_expected = format!(
                    "0x{}",
                    hash(&[&course.name, &section.name, &lesson.name,])
                );
                if slug_expected != lesson.slug {
                    progress.println(format!(
                        "  ╰─{}: {} ❌",
                        lesson.name.red().bold(),
                        lesson.slug.white()
                    ));

                    Self {
                        progress,
                        state: ValidatorStateV1::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                lesson.slug, slug_expected
                            ),
                        },
                        course,
                        tester,
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
                            state: ValidatorStateV1::Suite {
                                index_section,
                                index_lesson,
                                index_suite: 0,
                            },
                            course,
                            tester,
                        }
                    } else {
                        match (
                            index_section + 1 < tester.sections.len(),
                            index_lesson + 1 < section.lessons.len(),
                        ) {
                            (_, true) => Self {
                                progress,
                                state: ValidatorStateV1::Lesson {
                                    index_section,
                                    index_lesson: index_lesson + 1,
                                },
                                course,
                                tester,
                            },
                            (true, false) => Self {
                                progress,
                                state: ValidatorStateV1::Section {
                                    index_section: index_section + 1,
                                },
                                course,
                                tester,
                            },
                            (false, false) => Self {
                                progress,
                                state: ValidatorStateV1::Pass,
                                course,
                                tester,
                            },
                        }
                    }
                }
            }
            ValidatorStateV1::Suite {
                index_section,
                index_lesson,
                index_suite,
            } => {
                let section = &tester.sections[index_section];
                let lesson = &section.lessons[index_lesson];
                let suite = &lesson.suites.as_ref().expect(
                    "Suite has been checked to be Some in previous state",
                )[index_suite];

                let slug_expected = format!(
                    "0x{}",
                    hash(&[
                        &course.name,
                        &section.name,
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
                        state: ValidatorStateV1::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                suite.slug, slug_expected
                            ),
                        },
                        course,
                        tester,
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
                        state: ValidatorStateV1::Test {
                            index_section,
                            index_lesson,
                            index_suite,
                            index_test: 0,
                        },
                        course,
                        tester,
                    }
                }
            }
            ValidatorStateV1::Test {
                index_section,
                index_lesson,
                index_suite,
                index_test,
            } => {
                let section = &tester.sections[index_section];
                let lesson = &section.lessons[index_lesson];
                let suites = &lesson.suites.as_ref().expect(
                    "Suite has been checked to be Some in previous state",
                );
                let suite = &suites[index_suite];
                let test = &suite.tests[index_test];

                let slug_expected = format!(
                    "0x{}",
                    hash(&[
                        &course.name,
                        &section.name,
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
                        state: ValidatorStateV1::Fail {
                            reason: format!(
                                "Invalid slug: '{}', expected '{}'",
                                test.slug, slug_expected
                            ),
                        },
                        course,
                        tester,
                    }
                } else {
                    progress.println(format!(
                        "      ╰─{}: {} ✅",
                        test.name.green().bold(),
                        test.slug.white()
                    ));

                    progress.inc(1);

                    match (
                        index_section + 1 < tester.sections.len(),
                        index_lesson + 1 < section.lessons.len(),
                        index_suite + 1 < suites.len(),
                        index_test + 1 < suite.tests.len(),
                    ) {
                        (_, _, _, true) => Self {
                            progress,
                            state: ValidatorStateV1::Test {
                                index_section,
                                index_lesson,
                                index_suite,
                                index_test: index_test + 1,
                            },
                            course,
                            tester,
                        },
                        (_, _, true, false) => Self {
                            progress,
                            state: ValidatorStateV1::Suite {
                                index_section,
                                index_lesson,
                                index_suite: index_suite + 1,
                            },
                            course,
                            tester,
                        },
                        (_, true, false, false) => Self {
                            progress,
                            state: ValidatorStateV1::Lesson {
                                index_section,
                                index_lesson: index_lesson + 1,
                            },
                            course,
                            tester,
                        },
                        (true, false, false, false) => Self {
                            progress,
                            state: ValidatorStateV1::Section {
                                index_section: index_section + 1,
                            },
                            course,
                            tester,
                        },
                        (false, false, false, false) => Self {
                            progress,
                            state: ValidatorStateV1::Pass,
                            course,
                            tester,
                        },
                    }
                }
            }
            ValidatorStateV1::Fail { reason } => {
                progress.finish_and_clear();
                progress.println(format!("\n⚠ Error: {}", reason.red().bold()));

                Self {
                    progress,
                    state: ValidatorStateV1::Finish,
                    course,
                    tester,
                }
            }
            ValidatorStateV1::Pass => {
                progress.finish_and_clear();
                progress.println(
                    "\n🏁 Course format is valid".green().bold().to_string(),
                );

                Self {
                    progress,
                    state: ValidatorStateV1::Finish,
                    course,
                    tester,
                }
            }
            ValidatorStateV1::Finish => Self {
                progress,
                state: ValidatorStateV1::Finish,
                course,
                tester,
            },
        }
    }

    fn is_finished(&self) -> bool {
        self.state == ValidatorStateV1::Finish
    }
}
