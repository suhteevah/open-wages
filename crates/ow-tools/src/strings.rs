/// Extract ASCII strings of at least `min_len` printable characters from raw bytes.
pub fn find_strings(data: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let mut current = Vec::new();
    let mut start = 0;

    for (i, &b) in data.iter().enumerate() {
        if (32..=126).contains(&b) {
            if current.is_empty() {
                start = i;
            }
            current.push(b as char);
        } else {
            if current.len() >= min_len {
                results.push((start, current.iter().collect()));
            }
            current.clear();
        }
    }
    if current.len() >= min_len {
        results.push((start, current.iter().collect()));
    }

    results
}
