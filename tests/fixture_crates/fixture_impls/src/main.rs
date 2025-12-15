pub(crate) mod impl_in_file_module;

fn main() {
    let x = TestImplStruct {
        field_one: 1,
        field_two: String::from("two"),
    };

    x.func_test_one();
    x.func_test_two();
    x.func_test_three();
    TestImplStruct::func_test_four();
    x.func_test_five();

    println!("field_one: {}, field_two: {}", x.field_one, x.field_two);
}

pub struct TestImplStruct {
    field_one: i32,
    field_two: String,
}

impl TestImplStruct {
    fn func_test_one(&self) -> i32 {
        1
    }
}

impl TestImplStruct {
    fn func_test_two(&self) -> i32 {
        2
    }
}

mod nested_impl_block {
    use crate::TestImplStruct;

    impl TestImplStruct {
        pub(super) fn func_test_three(&self) -> i32 {
            3
        }
    }
}
