use serde::{Deserialize, Serialize};

/// Result of struct-pattern detection on a binary blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructPattern {
    pub stride: usize,
    pub record_count: usize,
    pub confidence: f64,
}

/// Try to detect fixed-size struct arrays by checking if null-byte positions repeat
/// at regular intervals. Returns the best match above 70% confidence, if any.
pub fn detect_repeating_struct(data: &[u8], max_stride: usize) -> Option<StructPattern> {
    if data.len() < 256 {
        return None;
    }

    let mut best: Option<StructPattern> = None;
    let mut best_score: f64 = 0.0;
    let limit = max_stride.min(data.len() / 4);

    for stride in 8..=limit {
        if !data.len().is_multiple_of(stride) {
            continue;
        }
        let count = data.len() / stride;
        if count < 3 {
            continue;
        }

        // Collect null-byte positions in the first record
        let null_positions: Vec<usize> = (0..stride)
            .filter(|&i| data[i] == 0)
            .collect();

        if null_positions.is_empty() {
            continue;
        }

        let mut matches = 0u64;
        let mut checks = 0u64;
        let records_to_check = count.min(10);

        for record in 1..records_to_check {
            for &pos in &null_positions {
                let idx = record * stride + pos;
                if idx < data.len() {
                    checks += 1;
                    if data[idx] == 0 {
                        matches += 1;
                    }
                }
            }
        }

        if checks > 0 {
            let score = matches as f64 / checks as f64;
            if score > best_score && score > 0.7 {
                best_score = score;
                best = Some(StructPattern {
                    stride,
                    record_count: count,
                    confidence: (score * 100.0).round() / 100.0,
                });
            }
        }
    }

    best
}
