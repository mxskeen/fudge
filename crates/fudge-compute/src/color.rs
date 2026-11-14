//! Color literal parsing: #rgb, #rrggbb, rgb(), hsl().

use crate::{AnswerKind, ComputeAnswer};

fn parse_hex(h: &str) -> Option<(u8, u8, u8)> {
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = |s: &str| u8::from_str_radix(s, 16).ok();
    match h.len() {
        3 => {
            let d = |i: usize| v(&h[i..=i]).map(|x| x * 17);
            Some((d(0)?, d(1)?, d(2)?))
        }
        6 => Some((v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?)),
        _ => None,
    }
}

fn func_args<'a>(s: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let rest = s.strip_prefix(name)?.trim_start();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    let parts: Vec<&str> = inner
        .split([',', ' ', '/'])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() == 3 {
        Some(parts)
    } else {
        None
    }
}

fn num(s: &str, suffix: &str) -> Option<f64> {
    let t = s.strip_suffix(suffix).unwrap_or(s);
    let v: f64 = t.parse().ok()?;
    v.is_finite().then_some(v)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 100.0) / 100.0;
    let l = l.clamp(0.0, 100.0) / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let f = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r1), f(g1), f(b1))
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u32, u32, u32) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let l = (max + min) / 2.0;
    let s = if d == 0.0 {
        0.0
    } else {
        d / (1.0 - (2.0 * l - 1.0).abs())
    };
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let h = (h + 360.0) % 360.0;
    (
        (h.round() as u32) % 360,
        (s * 100.0).round() as u32,
        (l * 100.0).round() as u32,
    )
}

pub fn evaluate(input: &str) -> Option<ComputeAnswer> {
    let s = input.trim().to_lowercase();
    let (r, g, b) = if let Some(h) = s.strip_prefix('#') {
        parse_hex(h)?
    } else if let Some(a) = func_args(&s, "rgb") {
        let mut out = [0u8; 3];
        for (i, p) in a.iter().enumerate() {
            let v = num(p, "")?.round();
            if !(0.0..=255.0).contains(&v) {
                return None;
            }
            out[i] = v as u8;
        }
        (out[0], out[1], out[2])
    } else if let Some(a) = func_args(&s, "hsl") {
        let h = num(a[0], "deg")?;
        let sat = num(a[1], "%")?;
        let l = num(a[2], "%")?;
        hsl_to_rgb(h, sat, l)
    } else {
        return None;
    };
    let hex = format!("#{r:02x}{g:02x}{b:02x}");
    let (h, sat, l) = rgb_to_hsl(r, g, b);
    Some(ComputeAnswer {
        kind: AnswerKind::Color,
        title: hex.clone(),
        subtitle: format!("rgb({r}, {g}, {b}) \u{b7} hsl({h}, {sat}%, {l}%)"),
        copy_text: hex.clone(),
        score: 95,
        needs_network: false,
        color_hex: Some(hex),
    })
}
