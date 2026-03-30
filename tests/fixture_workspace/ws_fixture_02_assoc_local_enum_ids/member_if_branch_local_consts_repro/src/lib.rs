pub struct S;

impl S {
    fn f() {
        if true {
            const Y: u8 = 0;
        } else {
            const Y: u8 = 1;
        }
    }
}

pub fn exercise_fixture() {
    S::f();
}
