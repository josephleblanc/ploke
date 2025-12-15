// Simple test file for system testing

pub fn hello_world() {
    println!("Hello from test_file!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        hello_world(); // Just call to ensure it compiles and runs
    }
}
