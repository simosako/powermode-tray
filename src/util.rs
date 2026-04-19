/// Encode a &str as a null-terminated UTF-16 Vec.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Encode a &str to a null-terminated UTF-16 array fitting in [u16; N].
pub fn to_wide_array<const N: usize>(s: &str) -> [u16; N] {
    let mut buf = [0u16; N];
    for (i, c) in s.encode_utf16().enumerate() {
        if i >= N - 1 {
            break;
        }
        buf[i] = c;
    }
    buf
}
