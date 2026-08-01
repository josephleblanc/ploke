//! Small vector helpers for embedding-diff mechanisms.

use super::error::ScoreError;

/// Euclidean norm of a finite vector.
pub fn norm(value: &[f64]) -> Result<f64, ScoreError> {
    if value.is_empty() {
        return Err(ScoreError::Empty);
    }
    if value.iter().any(|item| !item.is_finite()) {
        return Err(ScoreError::NonFinite);
    }

    Ok(value.iter().map(|item| item * item).sum::<f64>().sqrt())
}

/// Unit-normalizes a finite vector.
pub fn normalize(value: &[f64]) -> Result<Vec<f64>, ScoreError> {
    let magnitude = norm(value)?;
    if magnitude == 0.0 {
        return Err(ScoreError::ZeroTotal);
    }

    Ok(value.iter().map(|item| item / magnitude).collect())
}

/// Dot product of two finite, equally sized vectors.
pub fn dot(left: &[f64], right: &[f64]) -> Result<f64, ScoreError> {
    if left.len() != right.len() {
        return Err(ScoreError::LengthMismatch);
    }
    if left.is_empty() {
        return Err(ScoreError::Empty);
    }
    if left
        .iter()
        .chain(right.iter())
        .any(|item| !item.is_finite())
    {
        return Err(ScoreError::NonFinite);
    }

    Ok(left.iter().zip(right.iter()).map(|(a, b)| a * b).sum())
}
