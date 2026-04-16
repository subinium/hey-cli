//! ANSI escape code constants to eliminate duplication across modules.

pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const DIM_ITALIC: &str = "\x1b[2;3m";
pub(crate) const BOLD_WHITE: &str = "\x1b[1;97m";
pub(crate) const BOLD_GREEN: &str = "\x1b[1;32m";
pub(crate) const BOLD_RED: &str = "\x1b[1;31m";
pub(crate) const BOLD_YELLOW: &str = "\x1b[1;33m";
pub(crate) const BOLD_WHITE_BG: &str = "\x1b[1;37m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const GRAY: &str = "\x1b[90m";

// Background chips for risk badges
pub(crate) const BG_YELLOW_BLACK: &str = "\x1b[43;30m";
pub(crate) const BG_RED_WHITE: &str = "\x1b[41;97m";
pub(crate) const DIM_GRAY: &str = "\x1b[2;90m";
