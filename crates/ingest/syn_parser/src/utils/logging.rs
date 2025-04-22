// Color scheme constants (Tokyo Night inspired)
const COLOR_HEADER: Color = Color::TrueColor {
    r: 122,
    g: 162,
    b: 247,
}; // Soft blue
const COLOR_NAME: Color = Color::TrueColor {
    r: 255,
    g: 202,
    b: 158,
}; // Peach
const COLOR_ID: Color = Color::TrueColor {
    r: 187,
    g: 154,
    b: 247,
}; // Light purple
const COLOR_VIS: Color = Color::TrueColor {
    r: 158,
    g: 206,
    b: 255,
}; // Sky blue
const COLOR_PATH: Color = Color::TrueColor {
    r: 158,
    g: 206,
    b: 255,
}; // Sky blue
const COLOR_ERROR: Color = Color::TrueColor {
    r: 247,
    g: 118,
    b: 142,
}; // Soft red

pub use colored::Colorize;
use std::fmt::Debug;

use colored::{Color, ColoredString};

// ... (keep color constants)

/// Only implement for string types                                                 
#[allow(warnings)]
pub trait LogStyle: AsRef<str> {
    fn log_header(&self) -> ColoredString {
        self.as_ref().color(COLOR_HEADER).bold()
    }
    fn log_name(&self) -> ColoredString {
        self.as_ref().color(COLOR_NAME)
    }
    fn log_id(&self) -> ColoredString {
        self.as_ref().color(COLOR_ID)
    }
    fn log_vis(&self) -> ColoredString {
        self.as_ref().color(COLOR_VIS)
    }
    fn log_path(&self) -> ColoredString {
        self.as_ref().color(COLOR_PATH)
    }
    fn log_error(&self) -> ColoredString {
        self.as_ref().color(COLOR_ERROR).bold()
    }
    fn debug_fmt(&self) -> ColoredString
    where
        Self: Debug,
    {
        format!("{:?}", self).normal()
    }
}

impl<T: AsRef<str> + ?Sized> LogStyle for T {}

#[allow(warnings)]
pub trait LogStyleDebug: Debug {
    fn log_header_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_HEADER).bold()
    }
    fn log_name_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_NAME)
    }
    fn log_id_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_ID)
    }
    fn log_vis_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_VIS)
    }
    fn log_path_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_PATH)
    }
    fn log_error_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_ERROR).bold()
    }
}

impl<T: Debug> LogStyleDebug for T {}
