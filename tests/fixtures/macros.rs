// Sample with macro definitions

// Declaring a macro
macro_rules! say_hello {
    () => {
        println!("Hello, world!");
    };
}

// Declaring a generic macro
macro_rules! add {
    ($a:expr, $b:expr) => {
        $a + $b
    };
}

// Declaring a macro with multiple arguments
macro_rules! greet {
    ($name:expr) => {
        println!("Hello, {}", $name);
    };
    ($name:expr, $greeting:expr) => {
        println!("{}, {}", $greeting, $name);
    };
}

// Declaring a macro with arguments and repetition
macro_rules! repeat {
    ($($item:expr),*) => {
        $(
            println!("{}", $item);
        )*
    };
}

// Using a macro
say_hello!();

// Using a macro with arguments
let result = add!(5, 3);

// Using a macro with multiple patterns
greet!("Alice");
greet!("Alice", "Hi");

// Using a macro with repetition
repeat!(1, 2, 3, 4);
