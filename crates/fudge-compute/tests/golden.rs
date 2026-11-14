use fudge_compute::chrono::{DateTime, TimeZone, Utc};
use fudge_compute::{evaluate, wants_rates, AnswerKind, ComputeAnswer, Context, RateSource};
use std::collections::HashMap;

struct Mock;
impl RateSource for Mock {
    fn usd_rates(&self) -> Option<HashMap<String, f64>> {
        let mut m = HashMap::new();
        m.insert("INR".to_string(), 83.12);
        m.insert("EUR".to_string(), 0.92);
        Some(m)
    }
}

struct NoRates;
impl RateSource for NoRates {
    fn usd_rates(&self) -> Option<HashMap<String, f64>> {
        None
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 14, 8, 7, 0).unwrap()
}

fn run_with(input: &str, rates: Option<&dyn RateSource>, network: bool) -> Vec<ComputeAnswer> {
    let ctx = Context {
        now: now(),
        rates,
        network_enabled: network,
    };
    evaluate(input, &ctx)
}

fn run(input: &str) -> Vec<ComputeAnswer> {
    run_with(input, Some(&Mock), true)
}

fn first(input: &str) -> ComputeAnswer {
    run(input)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no answer for {input:?}"))
}

fn calc(input: &str, expected: &str) {
    let a = first(input);
    assert_eq!(a.kind, AnswerKind::Math, "{input}");
    assert_eq!(a.title, format!("= {expected}"), "{input}");
    assert_eq!(a.copy_text, expected, "{input}");
}

fn unit(input: &str, expected_title: &str) {
    let a = first(input);
    assert_eq!(a.kind, AnswerKind::Unit, "{input}");
    assert_eq!(a.title, expected_title, "{input}");
}

fn none(input: &str) {
    let r = run(input);
    assert!(r.is_empty(), "expected no answers for {input:?}, got {r:?}");
}

// ---------- calc ----------

#[test]
fn calc_golden() {
    let a = first("12 * 18");
    assert_eq!(a.kind, AnswerKind::Math);
    assert_eq!(a.title, "= 216");
    assert_eq!(a.copy_text, "216");
    assert_eq!(a.subtitle, "12 * 18");
    assert!(!a.needs_network);
    assert!(a.color_hex.is_none());
}

#[test]
fn calc_precedence_and_assoc() {
    calc("2 + 3 * 4", "14");
    calc("(2 + 3) * 4", "20");
    calc("2^3^2", "512");
    calc("-2^2", "-4");
    calc("2^-1", "0.5");
    calc("10 - 4 - 3", "3");
    calc("100 / 10 / 5", "2");
}

#[test]
fn calc_operators() {
    calc("7 / 2", "3.5");
    calc("1 / 3", "0.333333");
    calc("10 % 3", "1");
    calc("12 x 18", "216");
    calc("12 \u{d7} 18", "216");
    calc("144 \u{f7} 12", "12");
    calc("2(3+4)", "14");
    calc("5!", "120");
    calc("--3 + 1", "4");
}

#[test]
fn calc_percent_forms() {
    calc("15% of 80", "12");
    calc("80 + 15%", "92");
    calc("80 - 25%", "60");
    calc("200 * 10%", "20");
}

#[test]
fn calc_number_formats() {
    calc("1,000 + 1", "1001");
    calc("2,500,000 / 2", "1250000");
    calc("1e3 * 2", "2000");
    calc("0xff + 1", "256");
    calc("0b101 * 2", "10");
    calc("0.1 + 0.2", "0.3");
}

#[test]
fn calc_functions_and_constants() {
    calc("sqrt(16) + 1", "5");
    calc("sqrt 16 * 2", "8");
    calc("pi * 2", "6.283185");
    calc("2 * pi", "6.283185");
    calc("log2(8) + log(100)", "5");
    calc("ln(e)", "1");
    calc("abs(-5) * 2", "10");
    calc("round(2.6) + floor(1.9) + ceil(0.1)", "5");
    calc("cos(0) * 5", "5");
}

#[test]
fn calc_rejects_non_calculations() {
    none("42");
    none("-5");
    none("(42)");
    none("hello");
    none("1/0");
    none("5 % 0");
    none("sqrt(-1)");
    none("ln(0)");
    none("(((");
    none("1 +");
    none("2 3");
    none("0xzz");
    none("1.2.3");
    none("(1+2");
    none("171!");
}

// ---------- units ----------

#[test]
fn units_golden() {
    let a = first("72 f to c");
    assert_eq!(a.kind, AnswerKind::Unit);
    assert_eq!(a.title, "= 22.222222 \u{b0}C");
    assert_eq!(a.copy_text, "22.222222");
}

#[test]
fn units_compact_forms() {
    unit("72f to c", "= 22.222222 \u{b0}C");
    unit("5km in mi", "= 3.106856 mi");
    unit("5 KM IN MI", "= 3.106856 mi");
    unit("10 km as m", "= 10000 m");
}

#[test]
fn units_each_category() {
    unit("1 mile to km", "= 1.609344 km");
    unit("12 inches to cm", "= 30.48 cm");
    unit("6 feet to m", "= 1.8288 m");
    unit("2 kg to lb", "= 4.409245 lb");
    unit("5 pounds to ounces", "= 80 oz");
    unit("100 c to f", "= 212 \u{b0}F");
    unit("0 c to k", "= 273.15 K");
    unit("100 celsius to fahrenheit", "= 212 \u{b0}F");
    unit("1 gallon to liters", "= 3.785412 L");
    unit("1 acre to sqft", "= 43560 ft\u{b2}");
    unit("60 mph to km/h", "= 96.56064 km/h");
    unit("1 gb to mb", "= 1000 MB");
    unit("1 gib to mib", "= 1024 MiB");
    unit("1 hour to min", "= 60 min");
    unit("2 days to hours", "= 48 h");
    unit("1 atm to psi", "= 14.695949 psi");
    unit("1 kwh to kj", "= 3600 kJ");
    unit("1,000 m to km", "= 1 km");
}

#[test]
fn units_in_is_inch_not_separator_when_needed() {
    unit("5 in to cm", "= 12.7 cm");
}

#[test]
fn units_invalid() {
    none("5 kg to km");
    none("5 apples to pears");
    none("72 f to");
    none("f to c");
    none("to c");
}

// ---------- currency ----------

#[test]
fn currency_goldens() {
    let a = first("12 usd to inr");
    assert_eq!(a.kind, AnswerKind::Currency);
    assert_eq!(a.title, "= 997.44 INR");
    assert_eq!(a.subtitle, "1 USD = 83.12 INR \u{b7} rates from the internet");
    assert_eq!(a.copy_text, "997.44");
    assert!(a.needs_network);
    assert_eq!(first("90 eur to usd").title, "= 97.83 USD");
}

#[test]
fn currency_forms() {
    assert_eq!(first("12 USD TO INR").title, "= 997.44 INR");
    assert_eq!(first("12usd to inr").title, "= 997.44 INR");
    assert_eq!(first("$12 to inr").title, "= 997.44 INR");
    assert_eq!(first("12 dollars to rupees").title, "= 997.44 INR");
    assert_eq!(first("12 usd in eur").title, "= 11.04 EUR");
    let a = first("90 eur to usd");
    assert!(a.subtitle.starts_with("1 EUR = 1.08"), "{}", a.subtitle);
    assert!(a.subtitle.ends_with("rates from the internet"));
}

#[test]
fn currency_network_off() {
    let r = run_with("12 usd to inr", Some(&Mock), false);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].title, "Currency conversion is turned off");
    assert_eq!(r[0].subtitle, "Turn on network features in Settings \u{2192} Privacy");
    assert_eq!(r[0].score, 50);
}

#[test]
fn currency_no_rates() {
    for rates in [None, Some(&NoRates as &dyn RateSource)] {
        let r = run_with("12 usd to inr", rates, true);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Needs today's rates");
        assert_eq!(r[0].subtitle, "Connect to the internet to convert currency");
    }
}

#[test]
fn currency_wants_rates() {
    assert!(wants_rates("12 usd to inr"));
    assert!(wants_rates("$5 in eur"));
    assert!(!wants_rates("12 kg to lb"));
    assert!(!wants_rates("hello"));
    assert!(!wants_rates(""));
    assert!(!wants_rates("12 * 18"));
}

#[test]
fn currency_unknown_codes() {
    none("12 abc to xyz");
}

// ---------- time ----------

#[test]
fn time_golden() {
    let a = first("time tokyo");
    assert_eq!(a.kind, AnswerKind::Time);
    assert_eq!(a.title, "Tokyo: 5:07 PM (Sat, Nov 14)");
    assert_eq!(a.subtitle, "UTC+09:00 \u{b7} Asia/Tokyo");
    assert_eq!(a.copy_text, "5:07 PM");
}

#[test]
fn time_forms() {
    assert_eq!(first("tokyo time").title, "Tokyo: 5:07 PM (Sat, Nov 14)");
    assert_eq!(first("time in tokyo").title, "Tokyo: 5:07 PM (Sat, Nov 14)");
    assert_eq!(first("TIME Tokyo").title, "Tokyo: 5:07 PM (Sat, Nov 14)");
    assert_eq!(first("time asia/tokyo").title, "Tokyo: 5:07 PM (Sat, Nov 14)");
}

#[test]
fn time_utc_default() {
    let a = first("time");
    assert_eq!(a.title, "UTC: 8:07 AM (Sat, Nov 14)");
    assert_eq!(a.subtitle, "UTC+00:00 \u{b7} UTC");
}

#[test]
fn time_other_cities() {
    let a = first("time new york");
    assert_eq!(a.title, "New York: 3:07 AM (Sat, Nov 14)");
    assert_eq!(a.subtitle, "UTC-05:00 \u{b7} America/New_York");
    assert_eq!(first("nyc time").title, a.title);
    assert_eq!(first("time london").title, "London: 8:07 AM (Sat, Nov 14)");
    let m = first("time mumbai");
    assert_eq!(m.title, "Mumbai: 1:37 PM (Sat, Nov 14)");
    assert_eq!(m.subtitle, "UTC+05:30 \u{b7} Asia/Kolkata");
    assert_eq!(first("time sydney").subtitle, "UTC+11:00 \u{b7} Australia/Sydney");
    assert_eq!(first("time auckland").title, "Auckland: 9:07 PM (Sat, Nov 14)");
    assert_eq!(first("time sf").title, "San Francisco: 12:07 AM (Sat, Nov 14)");
}

#[test]
fn time_abbreviations() {
    assert_eq!(first("time jst").title, "JST: 5:07 PM (Sat, Nov 14)");
    assert_eq!(first("time ist").subtitle, "UTC+05:30 \u{b7} Asia/Kolkata");
}

#[test]
fn time_unknown_places() {
    none("time narnia");
    none("narnia time");
    none("time in ");
    none("time foo/bar");
}

// ---------- emoji ----------

#[test]
fn emoji_goldens() {
    let r = run("emoji shrug");
    assert!(!r.is_empty());
    assert_eq!(r[0].kind, AnswerKind::Emoji);
    assert_eq!(r[0].copy_text, "\u{1f937}");
    assert_eq!(r[0].title, "\u{1f937}");
    assert!(r[0].subtitle.ends_with("Enter copies"));
    let p = run("emoji party");
    assert!(p.iter().any(|a| a.copy_text == "\u{1f389}"), "{p:?}");
}

#[test]
fn emoji_shortcode_forms() {
    assert_eq!(first(":tada:").copy_text, "\u{1f389}");
    assert_eq!(first(":shrug:").copy_text, "\u{1f937}");
    assert!(run("emoji rocket").iter().any(|a| a.copy_text == "\u{1f680}"));
}

#[test]
fn emoji_limits_and_misses() {
    assert!(run("emoji face").len() <= 8);
    none("emoji");
    none("emoji zzzzqqqq");
    none(":");
    none("::");
}

// ---------- color ----------

#[test]
fn color_golden() {
    let a = first("#3b82f6");
    assert_eq!(a.kind, AnswerKind::Color);
    assert_eq!(a.title, "#3b82f6");
    assert_eq!(a.subtitle, "rgb(59, 130, 246) \u{b7} hsl(217, 91%, 60%)");
    assert_eq!(a.copy_text, "#3b82f6");
    assert_eq!(a.color_hex.as_deref(), Some("#3b82f6"));
}

#[test]
fn color_forms() {
    let a = first("#fff");
    assert_eq!(a.title, "#ffffff");
    assert_eq!(a.subtitle, "rgb(255, 255, 255) \u{b7} hsl(0, 0%, 100%)");
    assert_eq!(first("rgb(59, 130, 246)").title, "#3b82f6");
    assert_eq!(first("RGB(59,130,246)").color_hex.as_deref(), Some("#3b82f6"));
    assert_eq!(first("hsl(217, 91%, 60%)").color_hex.as_deref(), Some("#3c83f6"));
    let k = first("rgb(0,0,0)");
    assert_eq!(k.title, "#000000");
    assert_eq!(k.subtitle, "rgb(0, 0, 0) \u{b7} hsl(0, 0%, 0%)");
    assert_eq!(first("#FF0000").subtitle, "rgb(255, 0, 0) \u{b7} hsl(0, 100%, 50%)");
}

#[test]
fn color_invalid() {
    none("#zzz");
    none("#12");
    none("#12345");
    none("rgb(300,0,0)");
    none("rgb(1,2)");
    none("hsl(a,b,c)");
}

// ---------- misc ----------

#[test]
fn plain_words_and_blank() {
    none("");
    none("   ");
    none("firefox");
    none("hello world");
}

#[test]
fn answers_sorted_by_score() {
    let r = run("emoji party");
    assert!(r.windows(2).all(|w| w[0].score >= w[1].score));
}

#[test]
fn kind_serializes_snake_case() {
    let a = first("12 * 18");
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains("\"kind\":\"math\""), "{json}");
    let back: ComputeAnswer = serde_json::from_str(&json).unwrap();
    assert_eq!(back, a);
}

#[test]
fn never_panics_on_weird_input() {
    let long_parens = "(".repeat(10_000);
    let long_digits = "9".repeat(10_000);
    let deep = format!("{}1{}", "(".repeat(200), ")".repeat(200));
    let long_sum = "1+".repeat(200) + "1";
    let cases: Vec<&str> = vec![
        "",
        " ",
        "(((",
        ")))",
        "1/0",
        "0/0",
        "emoji",
        "emoji ",
        "time narnia",
        "time ",
        "time in",
        "#zzz",
        "#",
        "rgb(",
        "rgb()",
        "hsl(,,)",
        "hsl(1e999, 5%, 5%)",
        ":",
        ":::",
        "::a::",
        "\0",
        "\u{1f937}\u{1f937}",
        "\u{202e}12 * 18",
        "\u{e9}\u{e8}\u{ea}",
        "\u{65e5}\u{672c}\u{8a9e}",
        "9999999999999999999999^9999",
        "99999999999999999999999999999999999999999 * 99999999999999999999999999999999999999",
        "170!",
        "171!",
        "-1!",
        "1.5!",
        "2^1e308",
        "1e999",
        "1e-999 + 1",
        "% % %",
        "50%%",
        "of of of",
        "15% of",
        "x x x",
        "to to to",
        "5 to 5",
        "5 in in in",
        "$ to $",
        "$$ to $$",
        "1 usd to usd",
        "999999999999999999999 usd to inr",
        "nan",
        "inf",
        "NaN + 1",
        "0x",
        "0b",
        "0x\u{e9}",
        ".",
        "..",
        "1,",
        ",1",
        "1,00",
        "1,0000",
        "time /",
        "time //",
        "time a/b/c/d",
        "time \u{e9}/\u{e9}",
        "emoji \u{e9}",
        "emoji _",
        "emoji ----",
    ];
    for c in cases {
        let _ = run(c);
        let _ = run_with(c, None, false);
        let _ = wants_rates(c);
    }
    for c in [&long_parens, &long_digits, &deep, &long_sum] {
        let _ = run(c);
        let _ = wants_rates(c);
    }
}
