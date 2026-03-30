pub struct Formatter;

impl Formatter {
    fn line_width() {
        if true {
            const HARD_LINE_BREAK: u32 = 1;
            let _ = HARD_LINE_BREAK;
        } else {
            const HARD_LINE_BREAK: u32 = 2;
            let _ = HARD_LINE_BREAK;
        }
    }
}

pub fn exercise_fixture() {
    Formatter::line_width();
}
