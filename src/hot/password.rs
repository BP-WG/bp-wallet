// Taken from https://github.com/dewan-ahmed/PassMeRust/blob/main/src/entropy.rs

pub fn calculate_entropy(password: &str) -> f64 {
    let charset = calculate_charset(password);
    let length = password.len();

    length as f64 * charset.log2()
}

fn calculate_charset(password: &str) -> f64 {
    let mut charset = 0u32;

    if password.bytes().any(|byte| byte >= b'0' && byte <= b'9') {
        charset += 10; // Numbers
    }
    if password.bytes().any(|byte| byte >= b'a' && byte <= b'z') {
        charset += 26; // Lowercase letters
    }
    if password.bytes().any(|byte| byte >= b'A' && byte <= b'Z') {
        charset += 26; // Uppercase letters
    }
    if password.bytes().any(|byte| {
        byte < b'0' || (byte > b'9' && byte < b'A') || (byte > b'Z' && byte < b'a') || byte > b'z'
    }) {
        charset += 33; // Special characters, rough estimation
    }

    charset as f64
}
