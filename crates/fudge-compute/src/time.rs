//! World clock: "time tokyo", "new york time", "time in london".

use crate::{AnswerKind, ComputeAnswer};
use chrono::{DateTime, Datelike, Offset, Timelike, Utc};
use chrono_tz::Tz;

/// (aliases separated by '|', display name, IANA zone)
static CITIES: &[(&str, &str, &str)] = &[
    // Asia
    ("tokyo|osaka|kyoto", "Tokyo", "Asia/Tokyo"),
    ("seoul|busan", "Seoul", "Asia/Seoul"),
    ("beijing", "Beijing", "Asia/Shanghai"),
    ("shanghai", "Shanghai", "Asia/Shanghai"),
    ("shenzhen|guangzhou|chengdu|wuhan", "Shenzhen", "Asia/Shanghai"),
    ("hong kong|hongkong|hk", "Hong Kong", "Asia/Hong_Kong"),
    ("macau|macao", "Macau", "Asia/Macau"),
    ("taipei|taiwan", "Taipei", "Asia/Taipei"),
    ("singapore", "Singapore", "Asia/Singapore"),
    ("kuala lumpur", "Kuala Lumpur", "Asia/Kuala_Lumpur"),
    ("bangkok", "Bangkok", "Asia/Bangkok"),
    ("hanoi", "Hanoi", "Asia/Ho_Chi_Minh"),
    ("ho chi minh|ho chi minh city|saigon", "Ho Chi Minh City", "Asia/Ho_Chi_Minh"),
    ("jakarta", "Jakarta", "Asia/Jakarta"),
    ("manila", "Manila", "Asia/Manila"),
    ("yangon|rangoon", "Yangon", "Asia/Yangon"),
    ("mumbai|bombay", "Mumbai", "Asia/Kolkata"),
    ("delhi|new delhi", "Delhi", "Asia/Kolkata"),
    ("bangalore|bengaluru", "Bengaluru", "Asia/Kolkata"),
    ("kolkata|calcutta", "Kolkata", "Asia/Kolkata"),
    ("chennai|madras", "Chennai", "Asia/Kolkata"),
    ("hyderabad", "Hyderabad", "Asia/Kolkata"),
    ("pune", "Pune", "Asia/Kolkata"),
    ("ahmedabad", "Ahmedabad", "Asia/Kolkata"),
    ("kathmandu", "Kathmandu", "Asia/Kathmandu"),
    ("dhaka", "Dhaka", "Asia/Dhaka"),
    ("karachi", "Karachi", "Asia/Karachi"),
    ("lahore", "Lahore", "Asia/Karachi"),
    ("islamabad", "Islamabad", "Asia/Karachi"),
    ("colombo", "Colombo", "Asia/Colombo"),
    ("kabul", "Kabul", "Asia/Kabul"),
    ("tashkent", "Tashkent", "Asia/Tashkent"),
    ("almaty", "Almaty", "Asia/Almaty"),
    ("ulaanbaatar|ulan bator", "Ulaanbaatar", "Asia/Ulaanbaatar"),
    ("dubai", "Dubai", "Asia/Dubai"),
    ("abu dhabi", "Abu Dhabi", "Asia/Dubai"),
    ("doha", "Doha", "Asia/Qatar"),
    ("riyadh", "Riyadh", "Asia/Riyadh"),
    ("jeddah", "Jeddah", "Asia/Riyadh"),
    ("kuwait|kuwait city", "Kuwait City", "Asia/Kuwait"),
    ("baghdad", "Baghdad", "Asia/Baghdad"),
    ("tehran", "Tehran", "Asia/Tehran"),
    ("jerusalem|tel aviv", "Jerusalem", "Asia/Jerusalem"),
    ("beirut", "Beirut", "Asia/Beirut"),
    ("amman", "Amman", "Asia/Amman"),
    // Europe
    ("london|uk", "London", "Europe/London"),
    ("edinburgh", "Edinburgh", "Europe/London"),
    ("manchester", "Manchester", "Europe/London"),
    ("dublin", "Dublin", "Europe/Dublin"),
    ("lisbon", "Lisbon", "Europe/Lisbon"),
    ("paris", "Paris", "Europe/Paris"),
    ("berlin", "Berlin", "Europe/Berlin"),
    ("munich", "Munich", "Europe/Berlin"),
    ("frankfurt", "Frankfurt", "Europe/Berlin"),
    ("hamburg", "Hamburg", "Europe/Berlin"),
    ("madrid", "Madrid", "Europe/Madrid"),
    ("barcelona", "Barcelona", "Europe/Madrid"),
    ("rome", "Rome", "Europe/Rome"),
    ("milan", "Milan", "Europe/Rome"),
    ("amsterdam", "Amsterdam", "Europe/Amsterdam"),
    ("brussels", "Brussels", "Europe/Brussels"),
    ("vienna", "Vienna", "Europe/Vienna"),
    ("zurich", "Zurich", "Europe/Zurich"),
    ("geneva", "Geneva", "Europe/Zurich"),
    ("stockholm", "Stockholm", "Europe/Stockholm"),
    ("oslo", "Oslo", "Europe/Oslo"),
    ("copenhagen", "Copenhagen", "Europe/Copenhagen"),
    ("helsinki", "Helsinki", "Europe/Helsinki"),
    ("reykjavik", "Reykjavik", "Atlantic/Reykjavik"),
    ("athens", "Athens", "Europe/Athens"),
    ("warsaw", "Warsaw", "Europe/Warsaw"),
    ("prague", "Prague", "Europe/Prague"),
    ("budapest", "Budapest", "Europe/Budapest"),
    ("bucharest", "Bucharest", "Europe/Bucharest"),
    ("sofia", "Sofia", "Europe/Sofia"),
    ("belgrade", "Belgrade", "Europe/Belgrade"),
    ("kyiv|kiev", "Kyiv", "Europe/Kiev"),
    ("riga", "Riga", "Europe/Riga"),
    ("vilnius", "Vilnius", "Europe/Vilnius"),
    ("tallinn", "Tallinn", "Europe/Tallinn"),
    ("moscow", "Moscow", "Europe/Moscow"),
    ("saint petersburg|st petersburg", "Saint Petersburg", "Europe/Moscow"),
    ("istanbul", "Istanbul", "Europe/Istanbul"),
    // Africa
    ("cairo", "Cairo", "Africa/Cairo"),
    ("lagos", "Lagos", "Africa/Lagos"),
    ("abuja", "Abuja", "Africa/Lagos"),
    ("nairobi", "Nairobi", "Africa/Nairobi"),
    ("johannesburg|joburg", "Johannesburg", "Africa/Johannesburg"),
    ("cape town", "Cape Town", "Africa/Johannesburg"),
    ("casablanca", "Casablanca", "Africa/Casablanca"),
    ("algiers", "Algiers", "Africa/Algiers"),
    ("tunis", "Tunis", "Africa/Tunis"),
    ("accra", "Accra", "Africa/Accra"),
    ("dakar", "Dakar", "Africa/Dakar"),
    ("addis ababa", "Addis Ababa", "Africa/Addis_Ababa"),
    ("dar es salaam", "Dar es Salaam", "Africa/Dar_es_Salaam"),
    ("kampala", "Kampala", "Africa/Kampala"),
    ("kinshasa", "Kinshasa", "Africa/Kinshasa"),
    ("luanda", "Luanda", "Africa/Luanda"),
    ("harare", "Harare", "Africa/Harare"),
    // North America
    ("new york|nyc|new york city|ny", "New York", "America/New_York"),
    ("boston", "Boston", "America/New_York"),
    ("washington|washington dc|dc", "Washington, DC", "America/New_York"),
    ("philadelphia", "Philadelphia", "America/New_York"),
    ("miami", "Miami", "America/New_York"),
    ("atlanta", "Atlanta", "America/New_York"),
    ("detroit", "Detroit", "America/Detroit"),
    ("chicago", "Chicago", "America/Chicago"),
    ("dallas", "Dallas", "America/Chicago"),
    ("houston", "Houston", "America/Chicago"),
    ("austin", "Austin", "America/Chicago"),
    ("minneapolis", "Minneapolis", "America/Chicago"),
    ("denver", "Denver", "America/Denver"),
    ("salt lake city", "Salt Lake City", "America/Denver"),
    ("phoenix", "Phoenix", "America/Phoenix"),
    ("los angeles|la", "Los Angeles", "America/Los_Angeles"),
    ("san francisco|sf", "San Francisco", "America/Los_Angeles"),
    ("san diego", "San Diego", "America/Los_Angeles"),
    ("san jose", "San Jose", "America/Los_Angeles"),
    ("seattle", "Seattle", "America/Los_Angeles"),
    ("portland", "Portland", "America/Los_Angeles"),
    ("las vegas", "Las Vegas", "America/Los_Angeles"),
    ("anchorage", "Anchorage", "America/Anchorage"),
    ("honolulu|hawaii", "Honolulu", "Pacific/Honolulu"),
    ("toronto", "Toronto", "America/Toronto"),
    ("montreal", "Montreal", "America/Toronto"),
    ("ottawa", "Ottawa", "America/Toronto"),
    ("vancouver", "Vancouver", "America/Vancouver"),
    ("calgary|edmonton", "Calgary", "America/Edmonton"),
    ("winnipeg", "Winnipeg", "America/Winnipeg"),
    ("halifax", "Halifax", "America/Halifax"),
    ("mexico city", "Mexico City", "America/Mexico_City"),
    ("guadalajara", "Guadalajara", "America/Mexico_City"),
    ("havana", "Havana", "America/Havana"),
    ("panama|panama city", "Panama City", "America/Panama"),
    ("san juan", "San Juan", "America/Puerto_Rico"),
    ("kingston", "Kingston", "America/Jamaica"),
    ("guatemala city|guatemala", "Guatemala City", "America/Guatemala"),
    ("san jose cr|costa rica", "San Jose, Costa Rica", "America/Costa_Rica"),
    // South America
    ("sao paulo|s\u{e3}o paulo", "S\u{e3}o Paulo", "America/Sao_Paulo"),
    ("rio de janeiro|rio", "Rio de Janeiro", "America/Sao_Paulo"),
    ("buenos aires", "Buenos Aires", "America/Argentina/Buenos_Aires"),
    ("santiago", "Santiago", "America/Santiago"),
    ("lima", "Lima", "America/Lima"),
    ("bogota|bogot\u{e1}", "Bogot\u{e1}", "America/Bogota"),
    ("caracas", "Caracas", "America/Caracas"),
    ("montevideo", "Montevideo", "America/Montevideo"),
    ("la paz", "La Paz", "America/La_Paz"),
    // Oceania
    ("sydney", "Sydney", "Australia/Sydney"),
    ("melbourne", "Melbourne", "Australia/Melbourne"),
    ("brisbane", "Brisbane", "Australia/Brisbane"),
    ("perth", "Perth", "Australia/Perth"),
    ("adelaide", "Adelaide", "Australia/Adelaide"),
    ("canberra", "Canberra", "Australia/Sydney"),
    ("auckland", "Auckland", "Pacific/Auckland"),
    ("wellington", "Wellington", "Pacific/Auckland"),
    ("fiji|suva", "Suva", "Pacific/Fiji"),
];

/// (abbreviation, IANA zone, label)
static ABBREVIATIONS: &[(&str, &str, &str)] = &[
    ("utc", "UTC", "UTC"),
    ("gmt", "UTC", "GMT"),
    ("est", "America/New_York", "EST"),
    ("edt", "America/New_York", "EDT"),
    ("et", "America/New_York", "ET"),
    ("cst", "America/Chicago", "CST"),
    ("cdt", "America/Chicago", "CDT"),
    ("mst", "America/Denver", "MST"),
    ("mdt", "America/Denver", "MDT"),
    ("pst", "America/Los_Angeles", "PST"),
    ("pdt", "America/Los_Angeles", "PDT"),
    ("pt", "America/Los_Angeles", "PT"),
    ("ist", "Asia/Kolkata", "IST"),
    ("jst", "Asia/Tokyo", "JST"),
    ("kst", "Asia/Seoul", "KST"),
    ("cet", "Europe/Paris", "CET"),
    ("cest", "Europe/Paris", "CEST"),
    ("eet", "Europe/Helsinki", "EET"),
    ("bst", "Europe/London", "BST"),
    ("msk", "Europe/Moscow", "MSK"),
    ("aest", "Australia/Sydney", "AEST"),
    ("aedt", "Australia/Sydney", "AEDT"),
    ("sgt", "Asia/Singapore", "SGT"),
    ("hkt", "Asia/Hong_Kong", "HKT"),
];

fn normalize(s: &str) -> String {
    s.trim()
        .trim_end_matches(['?', '.', '!'])
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case_iana(s: &str) -> String {
    s.split('/')
        .map(|seg| {
            seg.split('_')
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join("_")
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Resolve a place string to (label, zone name, zone).
fn resolve(place: &str) -> Option<(String, &'static str, Tz)> {
    let spaced = place.replace('_', " ");
    for (aliases, display, tz) in CITIES {
        if aliases.split('|').any(|a| a == spaced) {
            let zone: Tz = tz.parse().ok()?;
            return Some((display.to_string(), *tz, zone));
        }
    }
    for (abbr, tz, label) in ABBREVIATIONS {
        if *abbr == place {
            let zone: Tz = tz.parse().ok()?;
            return Some((label.to_string(), *tz, zone));
        }
    }
    None
}

fn extract_place(q: &str) -> Option<&str> {
    if q == "time" || q == "current time" {
        return Some("utc");
    }
    for p in ["what time is it in ", "time in ", "time at ", "time for ", "time "] {
        if let Some(r) = q.strip_prefix(p) {
            return Some(r.trim());
        }
    }
    q.strip_suffix(" time").map(str::trim)
}

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn evaluate(input: &str, now: DateTime<Utc>) -> Option<ComputeAnswer> {
    let q = normalize(input);
    let place = extract_place(&q)?;
    if place.is_empty() {
        return None;
    }
    let (label, zone_name, zone) = match resolve(place) {
        Some(r) => r,
        None if place.contains('/') => {
            let name = title_case_iana(place);
            let zone: Tz = name.parse().ok()?;
            let label = name
                .rsplit('/')
                .next()
                .unwrap_or(&name)
                .replace('_', " ");
            (label, zone.name(), zone)
        }
        None => return None,
    };
    let dt = now.with_timezone(&zone);
    let h24 = dt.hour();
    let h12 = match h24 % 12 {
        0 => 12,
        h => h,
    };
    let ampm = if h24 < 12 { "AM" } else { "PM" };
    let clock = format!("{}:{:02} {}", h12, dt.minute(), ampm);
    let wd = WEEKDAYS[dt.weekday().num_days_from_monday() as usize];
    let mon = MONTHS[dt.month0() as usize];
    let secs = dt.offset().fix().local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let abs = secs.abs();
    let offset = format!("UTC{}{:02}:{:02}", sign, abs / 3600, (abs % 3600) / 60);
    Some(ComputeAnswer {
        kind: AnswerKind::Time,
        title: format!("{}: {} ({}, {} {})", label, clock, wd, mon, dt.day()),
        subtitle: format!("{} \u{b7} {}", offset, zone_name),
        copy_text: clock,
        score: 90,
        needs_network: false,
        color_hex: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_table_zones_parse() {
        for (_, _, tz) in CITIES {
            assert!(tz.parse::<Tz>().is_ok(), "bad tz {tz}");
        }
        for (_, tz, _) in ABBREVIATIONS {
            assert!(tz.parse::<Tz>().is_ok(), "bad tz {tz}");
        }
        assert!(CITIES.len() >= 140);
    }
}
