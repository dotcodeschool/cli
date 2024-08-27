use colored::Colorize;
use lazy_static::lazy_static;

pub const LOG: &str = "./dcs.log";

lazy_static! {
    pub static ref DOTCODESCHOOL: String =
        "[ DotCodeSchool CLI ]".bold().truecolor(230, 0, 122).to_string();
    pub static ref STAGGERED: String =
        "[ Staggered mode ]".bold().red().to_string();
    pub static ref OPTIONAL: String =
        "(optional)".white().dimmed().italic().to_string();
}
