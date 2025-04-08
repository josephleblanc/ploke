// A simple struct
pub struct Thing {
    pub value: i32,
}

// A simple function
pub fn do_thing(t: &Thing) -> i32 {
    // Added comment to change span slightly vs fixture_1
    t.value * 2
}
