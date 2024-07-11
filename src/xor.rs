const ENCRYPTION_KEY: &[u8] = b"BGTBuilder2024"; // Super secret key

#[allow(dead_code)]
pub(crate) fn xor_encrypt(data: &str) -> String {
    data.bytes()
        .zip(ENCRYPTION_KEY.iter().cycle())
        .map(|(b, &k)| (b ^ k).to_string())
        .collect::<Vec<String>>()
        .join(",")
}

pub(crate) fn xor_decrypt(data: &str) -> String {
    data.split(',')
        .map(|s| s.parse::<u8>().unwrap())
        .zip(ENCRYPTION_KEY.iter().cycle())
        .map(|(b, &k)| (b ^ k) as char)
        .collect()
}
