
/// Transform strings into different formats
pub trait StringTransform {
    /// Transforms a string into CamelCase
    ///
    /// Example: `"hello world" becomes "HelloWorld"`
    fn to_camel_case(&self) -> String;

    /// Transforms a string into snake_case
    ///
    /// Example: `"hello world" becomes "hello_world"`
    fn to_snake_case(&self) -> String;
}

impl StringTransform for str {
    fn to_camel_case(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map(|c| c.to_ascii_uppercase()).into_iter().collect::<String>() + chars.as_str()
            })
            .collect()
    }

    fn to_snake_case(&self) -> String {
        self.split_whitespace()
            .map(|word| word.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("_")
    }
}

impl StringTransform for String {
    fn to_camel_case(&self) -> String {
        self.as_str().to_camel_case()
    }

    fn to_snake_case(&self) -> String {
        self.as_str().to_snake_case()
    }
}

#[cfg(test)]
mod tests {
    use crate::util::string::StringTransform;

    #[test]
    fn test_to_camel_case() {
        assert_eq!("hello world".to_camel_case(), "HelloWorld");
        assert_eq!("rust programming language".to_camel_case(), "RustProgrammingLanguage");
        assert_eq!("singleword".to_camel_case(), "Singleword");
        assert_eq!("  leading and trailing  ".to_camel_case(), "LeadingAndTrailing");
        assert_eq!("".to_camel_case(), "");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!("hello world".to_snake_case(), "hello_world");
        assert_eq!("rust programming language".to_snake_case(), "rust_programming_language");
        assert_eq!("singleword".to_snake_case(), "singleword");
        assert_eq!("  leading and trailing  ".to_snake_case(), "leading_and_trailing");
        assert_eq!("".to_snake_case(), "");
    }
}
