pub struct Book;

impl Book {
    pub fn go() {
        { fn go_term() {} }
        { fn go_term() {} }
    }
}

pub fn exercise_fixture() {
    Book::go();
}
