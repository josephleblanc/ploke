// wait until thread is done before exiting
#[cfg(feature = "readline")]
editor_handle.join().unwrap();
