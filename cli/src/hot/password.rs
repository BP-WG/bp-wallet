// Taken from https://github.com/dewan-ahmed/PassMeRust/blob/main/src/entropy.rs

pub fn calculate_entropy(password: &str) -> f64 {
    let charset = calculate_charset(password);
    let length = password.len();

    length as f64 * charset.log2()
}

fn calculate_charset(password: &str) -> f64 {
    let mut charset = 0u32;

    if password.as_bytes().iter().any(u8::is_ascii_digit) {
        charset += 10; // Numbers
    }
    if password.as_bytes().iter().any(u8::is_ascii_lowercase) {
        charset += 26; // Lowercase letters
    }
    if password.as_bytes().iter().any(u8::is_ascii_uppercase) {
        charset += 26; // Uppercase letters
    }
    if !password.as_bytes().iter().all(u8::is_ascii_alphanumeric) {
        charset += 33; // Special characters, rough estimation
    }

    charset as f64
}
