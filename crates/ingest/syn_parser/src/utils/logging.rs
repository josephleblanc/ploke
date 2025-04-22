pub use colored::Colorize;
use std::fmt::Debug;

use colored::{Color, ColoredString}; // Import colored for terminal colors

use crate::parser::types::VisibilityKind;

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

// Logging trait for consistent styling
pub(crate) trait LogStyle: Colorize + Sized {
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
        self.as_str().color(COLOR_ERROR).bold()
    }
    // Convenience methods
    fn log_as(&self, style: &str) -> ColoredString {
        match style {
            "header" => self.log_header(),
            "name" => self.log_name(),
            "id" => self.log_id(),
            "vis" => self.log_vis(),
            "path" => self.log_path(),
            "error" => self.log_error(),
            _ => self.normal(),
        }
    }
}

impl LogStyle for String {}
impl LogStyle for &str {}

// // Blanket implementation for all string types
// impl<T: AsRef<str> + Colorize> LogStyle for T {
//     // All methods use the default implementations above
// }

// Specialized implementations
impl LogStyle for VisibilityKind {
    fn log_vis(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_VIS)
    }

    // Can override other methods if needed
}

// impl LogStyle for str {
//     fn log_header(&self) -> ColoredString {
//         self.color(COLOR_HEADER).bold()
//     }
//     fn log_name(&self) -> ColoredString {
//         self.color(COLOR_NAME)
//     }
//     fn log_id(&self) -> ColoredString {
//         self.color(COLOR_ID)
//     }
//     fn log_vis(&self) -> ColoredString {
//         self.color(COLOR_VIS)
//     }
//     fn log_path(&self) -> ColoredString {
//         self.color(COLOR_PATH)
//     }
//     fn log_error(&self) -> ColoredString {
//         self.color(COLOR_ERROR).bold()
//     }
// }

// impl LogStyle for VisibilityKind {
//     fn log_vis(&self) -> ColoredString {
//         format!("{:?}", self).color(COLOR_VIS)
//     }
//
//     // ... other trait methods can have default implementations
//     fn log_header(&self) -> ColoredString {
//         todo!()
//     }
//
//     fn log_name(&self) -> ColoredString {
//         todo!()
//     }
//
//     fn log_id(&self) -> ColoredString {
//         todo!()
//     }
//
//     fn log_path(&self) -> ColoredString {
//         todo!()
//     }
//
//     fn log_error(&self) -> ColoredString {
//         todo!()
//     }
// }
