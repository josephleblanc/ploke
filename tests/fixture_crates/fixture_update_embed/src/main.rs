mod other_mod;
// use other_mod::OtherStruct;

fn main() {
    println!("Hello, world!");
    let some_struct = TestStruct(5);
    let other_field = func_with_params(2, some_struct);
    // let something = OtherStruct { other_field };
    // println!("The other field is: {}", something.other_field);
    let num_four = other_mod::simple_four();
    println!("num_four: {num_four}, number two: {}", inner_test_mod::double_inner_mod::STR_TWO);
}

fn func_with_params(x: i32, y: TestStruct) -> u32 {
    let one = x.saturating_mul(x) as u32;
    let two: u32 = y.0.unsigned_abs();
    one + two + inner_test_mod::NUMBER_ONE
}

pub mod inner_test_mod {
    pub const NUMBER_ONE: u32 = 1;
    pub mod double_inner_mod {
        pub static STR_TWO: &str = "two";
    }
}

struct TestStruct(pub i32);