use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
