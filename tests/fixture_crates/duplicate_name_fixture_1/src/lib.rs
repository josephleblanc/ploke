// A simple struct
pub struct Thing {
    pub value: i32,
}

// A simple function
pub fn do_thing(t: &Thing) -> i32 {
    t.value * 2
}
