#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use syn_parser::parser::channel::ParserMessage;
    use syn_parser::parser::channel::{create_bounded_parser_channel, create_parser_channel};
    use syn_parser::parser::visitor::start_parser_worker;

    #[test]
    fn test_flume_based_parsing() {
        // Create channels for communication
        let (sender, receiver) = create_parser_channel();
        let (result_sender, result_receiver) = create_parser_channel();

        // Start the parser worker in a background thread
        let worker = start_parser_worker(receiver, result_sender);

        // Send a file to parse
        let fixture_path = PathBuf::from("tests/fixtures/functions.rs");
        sender.send(ParserMessage::ParseFile(fixture_path)).unwrap();

        // Send shutdown signal
        sender.send(ParserMessage::Shutdown).unwrap();

        // Wait for the result
        if let ParserMessage::ParseResult(result) = result_receiver.recv().unwrap() {
            let code_graph = result.expect("Failed to parse file");
            assert!(
                !code_graph.functions.is_empty(),
                "No functions found in the parsed file"
            );
        } else {
            panic!("Expected ParseResult message");
        }

        // Wait for the worker to finish
        worker.join().unwrap();
    }

    #[test]
    fn test_multiple_workers_concurrent_parsing() {
        // Create bounded channels for communication
        let (sender, receiver) = create_bounded_parser_channel(10);
        let (result_sender, result_receiver) = create_parser_channel();

        // Create shared sender for all workers
        let sender = Arc::new(sender);
        let result_sender = Arc::new(result_sender);

        // Number of worker threads to spawn
        const NUM_WORKERS: usize = 3;

        // Create a barrier to synchronize worker startup
        let barrier = Arc::new(Barrier::new(NUM_WORKERS));

        // Spawn multiple worker threads
        let mut workers = Vec::with_capacity(NUM_WORKERS);

        for _ in 0..NUM_WORKERS {
            let receiver_clone = receiver.clone();
            let result_sender_clone = Arc::clone(&result_sender);
            let barrier_clone = Arc::clone(&barrier);

            let worker = thread::spawn(move || {
                // Wait for all workers to be ready
                barrier_clone.wait();

                // Start the parser worker
                // FIX: Clone the sender from inside the Arc instead of passing the Arc itself
                start_parser_worker(receiver_clone, (*result_sender_clone).clone());
            });

            workers.push(worker);
        }

        // Test files to parse
        let test_files = vec![
            "tests/fixtures/functions.rs",
            "tests/fixtures/traits.rs",
            "tests/fixtures/modules.rs",
            "tests/fixtures/macros.rs",
        ];

        // Send files to parse
        for file in test_files {
            let path = PathBuf::from(file);
            sender.send(ParserMessage::ParseFile(path)).unwrap();
        }

        // Send shutdown signals (one for each worker)
        for _ in 0..NUM_WORKERS {
            sender.send(ParserMessage::Shutdown).unwrap();
        }

        // Collect and verify results
        let mut result_count = 0;
        while let Ok(message) = result_receiver.recv() {
            if let ParserMessage::ParseResult(result) = message {
                let code_graph = result.expect("Failed to parse file");
                assert!(
                    !code_graph.type_graph.is_empty() || !code_graph.functions.is_empty(),
                    "Empty graph returned from parsing"
                );
                result_count += 1;

                // Break once we've received all expected results
                if result_count >= test_files.len() {
                    break;
                }
            }
        }

        assert_eq!(
            result_count,
            test_files.len(),
            "Did not receive expected number of parsing results"
        );

        // Wait for all workers to finish
        for worker in workers {
            worker.join().unwrap();
        }
    }
}
