//! Number formatting helpers shared by all modules.

/// Up to 6 decimals, trailing zeros trimmed, never "-0".
pub(crate) fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v.abs() >= 1e15 {
        return format!("{v:e}");
    }
    let mut s = format!("{v:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Fixed number of decimals, never "-0.00".
pub(crate) fn fmt_fixed(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.starts_with('-') && s.chars().all(|c| matches!(c, '-' | '0' | '.')) {
        return s[1..].to_string();
    }
    s
}

/// Parse a leading number (sign, thousands separators, decimals, exponent).
/// Returns the value and the unparsed remainder.
pub(crate) fn parse_number_prefix(s: &str) -> Option<(f64, &str)> {
    let b = s.as_bytes();
    let n = b.len();
    let mut i = 0;
    let mut out = String::new();
    if i < n && (b[i] == b'-' || b[i] == b'+') {
        if b[i] == b'-' {
            out.push('-');
        }
        i += 1;
    }
    let int_start = i;
    while i < n && b[i].is_ascii_digit() {
        out.push(b[i] as char);
        i += 1;
    }
    let int_len = i - int_start;
    if (1..=3).contains(&int_len) && b[int_start] != b'0' {
        while i + 3 < n
            && b[i] == b','
            && b[i + 1..=i + 3].iter().all(|c| c.is_ascii_digit())
            && !(i + 4 < n && b[i + 4].is_ascii_digit())
        {
            for k in 1..=3 {
                out.push(b[i + k] as char);
            }
            i += 4;
        }
    }
    let mut frac = 0;
    if i + 1 < n && b[i] == b'.' && b[i + 1].is_ascii_digit() {
        out.push('.');
        i += 1;
        while i < n && b[i].is_ascii_digit() {
            out.push(b[i] as char);
            i += 1;
            frac += 1;
        }
    }
    if int_len == 0 && frac == 0 {
        return None;
    }
    if i < n && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        let mut exp = String::from("e");
        if j < n && (b[j] == b'+' || b[j] == b'-') {
            exp.push(b[j] as char);
            j += 1;
        }
        if j < n && b[j].is_ascii_digit() {
            while j < n && b[j].is_ascii_digit() {
                exp.push(b[j] as char);
                j += 1;
            }
            out.push_str(&exp);
            i = j;
        }
    }
    let v: f64 = out.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some((v, &s[i..]))
}
