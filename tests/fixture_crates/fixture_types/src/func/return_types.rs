use std::fmt::Display;

use crate::{MathOperation, Point};

pub(crate) fn consumes_point(point: Point) -> bool {
    point.0 != point.1
}

pub(crate) fn generic_func<T: Display + Clone, S: Send + Sync>(first: T, unused_param: S) -> T {
    println!("T is: {}", first);
    first
}

fn math_operation_consumer(func_param: MathOperation, x: i32, y: i32) -> i32 {
    func_param(x, y)
}

fn math_operation_producer() -> MathOperation {
    (|x, y| x + y) as _
}

pub(in crate::func) mod restricted_duplicate {
    use std::fmt::Display;

    use crate::{MathOperation, Point};

    pub(crate) fn consumes_point(point: Point) -> bool {
        point.0 != point.1
    }

    pub(crate) fn generic_func<T: Display + Clone, S: Send + Sync>(first: T, unused_param: S) -> T {
        println!("T is: {}", first);
        first
    }

    fn math_operation_consumer(func_param: MathOperation, x: i32, y: i32) -> i32 {
        func_param(x, y)
    }

    fn math_operation_producer() -> MathOperation {
        (|x, y| x + y) as _
    }
}
