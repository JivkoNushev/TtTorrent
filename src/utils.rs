/// Converts a slice of bytes into a string and prints it to the console.
///
/// This function takes a reference to a slice of bytes `input` and converts it into a
/// `String` by interpreting each byte as a character and collecting them. The resulting
/// string is then printed to the console.
///
/// # Arguments
///
/// * `input` - A reference to a slice of bytes to be converted and printed.
///
/// # Example
///
/// ```rust
/// use my_project::print_bytes_as_string;
///
/// let bytes = &[72, 101, 108, 108, 111]; // Corresponds to "Hello" in ASCII
/// print_bytes_as_string(bytes);
/// ```
///
/// In this example, the `print_bytes_as_string` function converts a slice of bytes
/// representing the ASCII characters for "Hello" into a string and prints it.
pub fn print_bytes_as_string(input: &[u8]) {
    let bytes_as_str: String = input.iter().map(|&c| c as char).collect();

    println!("{bytes_as_str}");
}
