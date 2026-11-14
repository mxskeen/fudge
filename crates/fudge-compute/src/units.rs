//! Unit conversion: "<num> <unit> to <unit>".

use crate::format::{fmt_num, parse_number_prefix};
use crate::{AnswerKind, ComputeAnswer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dim {
    Length,
    Mass,
    Temp,
    Volume,
    Area,
    Speed,
    Data,
    Time,
    Pressure,
    Energy,
}

struct U {
    names: &'static [&'static str],
    dim: Dim,
    /// base = (value + offset) * factor
    factor: f64,
    offset: f64,
    disp: &'static str,
}

macro_rules! u {
    ($names:expr, $dim:ident, $f:expr, $disp:expr) => {
        U {
            names: $names,
            dim: Dim::$dim,
            factor: $f,
            offset: 0.0,
            disp: $disp,
        }
    };
}

static UNITS: &[U] = &[
    // length (base: metre)
    u!(&["mm", "millimeter", "millimetre"], Length, 0.001, "mm"),
    u!(&["cm", "centimeter", "centimetre"], Length, 0.01, "cm"),
    u!(&["m", "meter", "metre"], Length, 1.0, "m"),
    u!(&["km", "kilometer", "kilometre"], Length, 1000.0, "km"),
    u!(&["in", "inch", "inches"], Length, 0.0254, "in"),
    u!(&["ft", "foot", "feet"], Length, 0.3048, "ft"),
    u!(&["yd", "yard"], Length, 0.9144, "yd"),
    u!(&["mi", "mile"], Length, 1609.344, "mi"),
    u!(&["nmi", "nautical mile"], Length, 1852.0, "nmi"),
    // mass (base: kilogram)
    u!(&["mg", "milligram"], Mass, 0.000_001, "mg"),
    u!(&["g", "gram"], Mass, 0.001, "g"),
    u!(&["kg", "kilogram", "kilo"], Mass, 1.0, "kg"),
    u!(&["t", "tonne", "metric ton"], Mass, 1000.0, "t"),
    u!(&["oz", "ounce"], Mass, 0.028_349_523_125, "oz"),
    u!(&["lb", "pound"], Mass, 0.453_592_37, "lb"),
    u!(&["st", "stone"], Mass, 6.350_293_18, "st"),
    // temperature (base: kelvin)
    U {
        names: &["c", "celsius", "centigrade"],
        dim: Dim::Temp,
        factor: 1.0,
        offset: 273.15,
        disp: "\u{b0}C",
    },
    U {
        names: &["f", "fahrenheit"],
        dim: Dim::Temp,
        factor: 5.0 / 9.0,
        offset: 459.67,
        disp: "\u{b0}F",
    },
    u!(&["k", "kelvin"], Temp, 1.0, "K"),
    // volume (base: litre)
    u!(&["ml", "milliliter", "millilitre"], Volume, 0.001, "mL"),
    u!(&["cl", "centiliter", "centilitre"], Volume, 0.01, "cL"),
    u!(&["l", "liter", "litre"], Volume, 1.0, "L"),
    u!(&["gal", "gallon"], Volume, 3.785_411_784, "gal"),
    u!(&["qt", "quart"], Volume, 0.946_352_946, "qt"),
    u!(&["pt", "pint"], Volume, 0.473_176_473, "pt"),
    u!(&["cup"], Volume, 0.236_588_236_5, "cup"),
    u!(
        &["floz", "fl oz", "fluid ounce"],
        Volume,
        0.029_573_529_562_5,
        "fl oz"
    ),
    u!(&["tsp", "teaspoon"], Volume, 0.004_928_921_593_75, "tsp"),
    u!(&["tbsp", "tablespoon"], Volume, 0.014_786_764_781_25, "tbsp"),
    // area (base: square metre)
    u!(
        &["m2", "m\u{b2}", "sqm", "sq m", "square meter", "square metre"],
        Area,
        1.0,
        "m\u{b2}"
    ),
    u!(&["cm2", "cm\u{b2}", "square centimeter"], Area, 0.0001, "cm\u{b2}"),
    u!(
        &["km2", "km\u{b2}", "sqkm", "sq km", "square kilometer", "square kilometre"],
        Area,
        1_000_000.0,
        "km\u{b2}"
    ),
    u!(
        &["ft2", "ft\u{b2}", "sqft", "sq ft", "square foot", "square feet"],
        Area,
        0.092_903_04,
        "ft\u{b2}"
    ),
    u!(&["yd2", "yd\u{b2}", "square yard"], Area, 0.836_127_36, "yd\u{b2}"),
    u!(
        &["mi2", "mi\u{b2}", "sqmi", "sq mi", "square mile"],
        Area,
        2_589_988.110_336,
        "mi\u{b2}"
    ),
    u!(&["acre"], Area, 4046.856_422_4, "acre"),
    u!(&["ha", "hectare"], Area, 10_000.0, "ha"),
    // speed (base: m/s)
    u!(&["m/s", "mps", "meter per second"], Speed, 1.0, "m/s"),
    u!(&["km/h", "kmh", "kph", "kmph", "kilometer per hour"], Speed, 1.0 / 3.6, "km/h"),
    u!(&["mph", "mi/h", "mile per hour"], Speed, 0.447_04, "mph"),
    u!(&["ft/s", "fps"], Speed, 0.3048, "ft/s"),
    u!(&["knot", "kn", "kt"], Speed, 1852.0 / 3600.0, "kn"),
    // data (base: byte)
    u!(&["b", "byte"], Data, 1.0, "B"),
    u!(&["kb", "kilobyte"], Data, 1e3, "kB"),
    u!(&["mb", "megabyte"], Data, 1e6, "MB"),
    u!(&["gb", "gigabyte"], Data, 1e9, "GB"),
    u!(&["tb", "terabyte"], Data, 1e12, "TB"),
    u!(&["kib", "kibibyte"], Data, 1024.0, "KiB"),
    u!(&["mib", "mebibyte"], Data, 1_048_576.0, "MiB"),
    u!(&["gib", "gibibyte"], Data, 1_073_741_824.0, "GiB"),
    u!(&["tib", "tebibyte"], Data, 1_099_511_627_776.0, "TiB"),
    u!(&["bit"], Data, 0.125, "bit"),
    u!(&["kbit", "kilobit"], Data, 125.0, "kbit"),
    u!(&["mbit", "megabit"], Data, 125_000.0, "Mbit"),
    u!(&["gbit", "gigabit"], Data, 125_000_000.0, "Gbit"),
    // time (base: second)
    u!(&["ms", "millisecond", "msec"], Time, 0.001, "ms"),
    u!(&["s", "sec", "second"], Time, 1.0, "s"),
    u!(&["min", "minute"], Time, 60.0, "min"),
    u!(&["h", "hr", "hour"], Time, 3600.0, "h"),
    u!(&["day", "d"], Time, 86_400.0, "day"),
    u!(&["week", "wk"], Time, 604_800.0, "week"),
    u!(&["year", "yr"], Time, 31_557_600.0, "year"),
    // pressure (base: pascal)
    u!(&["pa", "pascal"], Pressure, 1.0, "Pa"),
    u!(&["hpa", "hectopascal"], Pressure, 100.0, "hPa"),
    u!(&["kpa", "kilopascal"], Pressure, 1000.0, "kPa"),
    u!(&["mbar", "millibar"], Pressure, 100.0, "mbar"),
    u!(&["bar"], Pressure, 100_000.0, "bar"),
    u!(&["psi"], Pressure, 6894.757_293_168, "psi"),
    u!(&["atm", "atmosphere"], Pressure, 101_325.0, "atm"),
    u!(&["mmhg"], Pressure, 133.322_387_415, "mmHg"),
    // energy (base: joule)
    u!(&["j", "joule"], Energy, 1.0, "J"),
    u!(&["kj", "kilojoule"], Energy, 1000.0, "kJ"),
    u!(&["cal", "calorie"], Energy, 4.184, "cal"),
    u!(&["kcal", "kilocalorie"], Energy, 4184.0, "kcal"),
    u!(&["wh", "watt hour"], Energy, 3600.0, "Wh"),
    u!(&["kwh", "kilowatt hour"], Energy, 3_600_000.0, "kWh"),
];

fn normalize(name: &str) -> String {
    let lower: String = name.to_lowercase().replace('\u{b0}', "");
    let collapsed = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    for p in ["degrees ", "degree ", "deg "] {
        if let Some(rest) = collapsed.strip_prefix(p) {
            return rest.to_string();
        }
    }
    collapsed
}

fn exact(n: &str) -> Option<&'static U> {
    UNITS.iter().find(|u| u.names.iter().any(|x| *x == n))
}

fn find(name: &str) -> Option<&'static U> {
    let n = normalize(name);
    if n.is_empty() {
        return None;
    }
    exact(&n)
        .or_else(|| n.strip_suffix("es").and_then(exact))
        .or_else(|| n.strip_suffix('s').and_then(exact))
}

fn to_base(u: &U, v: f64) -> f64 {
    (v + u.offset) * u.factor
}

fn from_base(u: &U, b: f64) -> f64 {
    b / u.factor - u.offset
}

fn is_sep(tok: &str) -> bool {
    matches!(tok.to_lowercase().as_str(), "to" | "in" | "as" | "into")
}

fn try_convert(left: &str, right: &str) -> Option<ComputeAnswer> {
    let (v, rest) = parse_number_prefix(left.trim())?;
    let from = find(rest.trim())?;
    let to = find(right.trim())?;
    if from.dim != to.dim {
        return None;
    }
    let out = from_base(to, to_base(from, v));
    if !out.is_finite() {
        return None;
    }
    let s = fmt_num(out);
    Some(ComputeAnswer {
        kind: AnswerKind::Unit,
        title: format!("= {} {}", s, to.disp),
        subtitle: format!("{} {} \u{2192} {}", fmt_num(v), from.disp, to.disp),
        copy_text: s,
        score: 90,
        needs_network: false,
        color_hex: None,
    })
}

pub fn evaluate(input: &str) -> Option<ComputeAnswer> {
    let toks: Vec<&str> = input.split_whitespace().collect();
    if toks.len() < 3 {
        return None;
    }
    for i in 1..toks.len() - 1 {
        if is_sep(toks[i]) {
            let left = toks[..i].join(" ");
            let right = toks[i + 1..].join(" ");
            if let Some(a) = try_convert(&left, &right) {
                return Some(a);
            }
        }
    }
    None
}
