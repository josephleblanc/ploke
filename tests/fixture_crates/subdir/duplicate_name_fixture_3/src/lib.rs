// A simple struct (Identical content to fixture_1)
pub struct Thing {
    pub value: i32,
}

// A simple function (Identical content to fixture_1)
pub fn do_thing(t: &Thing) -> i32 {
    t.value * 2
}
