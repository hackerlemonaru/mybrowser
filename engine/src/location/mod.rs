//! Location Profile System — Perfect, Undetectable Geolocation Spoofing
//!
//! Every location signal is spoofed consistently so that no mismatch can be
//! detected by anti-fingerprinting checks. All values agree with the chosen
//! geographic location and the proxy exit IP country.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;

/// A complete geographic location profile. All fields are mutually consistent:
/// timezone matches country, locale matches language, currency matches country,
/// measurement system matches region, phone code matches country, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationProfile {
    /// Human-readable name (e.g. "New York, USA").
    pub name: String,
    /// ISO 3166-1 alpha-2 country code (e.g. "US").
    pub country_code: String,
    /// ISO 3166-1 alpha-3 country code (e.g. "USA").
    pub country_code_3: String,
    /// Country name (e.g. "United States").
    pub country_name: String,
    /// Region / state / province (e.g. "New York").
    pub region: String,
    /// City (e.g. "New York").
    pub city: String,
    /// Latitude.
    pub latitude: f64,
    /// Longitude.
    pub longitude: f64,
    /// IANA timezone (e.g. "America/New_York").
    pub timezone: String,
    /// Primary language tag (e.g. "en").
    pub language: String,
    /// BCP-47 locale (e.g. "en-US").
    pub locale: String,
    /// Accept-Language header value (e.g. "en-US,en;q=0.9").
    pub accept_language: String,
    /// ISO 4217 currency code (e.g. "USD").
    pub currency: String,
    /// Currency symbol (e.g. "$").
    pub currency_symbol: String,
    /// Measurement system: "metric" or "imperial".
    pub measurement_system: String,
    /// Phone country code (e.g. "1").
    pub phone_code: String,
    /// Date format (e.g. "MM/DD/YYYY").
    pub date_format: String,
    /// Time format: "12" or "24".
    pub time_format: String,
    /// First day of week: 0=Sunday, 1=Monday.
    pub first_day_of_week: u8,
    /// Driving side: "left" or "right".
    pub driving_side: String,
    /// Voltage (e.g. 120 or 230).
    pub voltage: u16,
    /// Plug type (e.g. "A/B").
    pub plug_type: String,
}

/// Predefined location catalog — major cities worldwide, all internally
/// consistent (timezone ↔ country ↔ locale ↔ currency ↔ measurement).
pub fn location_catalog() -> Vec<LocationProfile> {
    vec![
        // ── United States ──
        LocationProfile {
            name: "New York, USA".to_string(),
            country_code: "US".to_string(),
            country_code_3: "USA".to_string(),
            country_name: "United States".to_string(),
            region: "New York".to_string(),
            city: "New York".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            timezone: "America/New_York".to_string(),
            language: "en".to_string(),
            locale: "en-US".to_string(),
            accept_language: "en-US,en;q=0.9".to_string(),
            currency: "USD".to_string(),
            currency_symbol: "$".to_string(),
            measurement_system: "imperial".to_string(),
            phone_code: "1".to_string(),
            date_format: "MM/DD/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 120,
            plug_type: "A/B".to_string(),
        },
        LocationProfile {
            name: "Los Angeles, USA".to_string(),
            country_code: "US".to_string(),
            country_code_3: "USA".to_string(),
            country_name: "United States".to_string(),
            region: "California".to_string(),
            city: "Los Angeles".to_string(),
            latitude: 34.0522,
            longitude: -118.2437,
            timezone: "America/Los_Angeles".to_string(),
            language: "en".to_string(),
            locale: "en-US".to_string(),
            accept_language: "en-US,en;q=0.9".to_string(),
            currency: "USD".to_string(),
            currency_symbol: "$".to_string(),
            measurement_system: "imperial".to_string(),
            phone_code: "1".to_string(),
            date_format: "MM/DD/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 120,
            plug_type: "A/B".to_string(),
        },
        LocationProfile {
            name: "Chicago, USA".to_string(),
            country_code: "US".to_string(),
            country_code_3: "USA".to_string(),
            country_name: "United States".to_string(),
            region: "Illinois".to_string(),
            city: "Chicago".to_string(),
            latitude: 41.8781,
            longitude: -87.6298,
            timezone: "America/Chicago".to_string(),
            language: "en".to_string(),
            locale: "en-US".to_string(),
            accept_language: "en-US,en;q=0.9".to_string(),
            currency: "USD".to_string(),
            currency_symbol: "$".to_string(),
            measurement_system: "imperial".to_string(),
            phone_code: "1".to_string(),
            date_format: "MM/DD/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 120,
            plug_type: "A/B".to_string(),
        },
        LocationProfile {
            name: "Miami, USA".to_string(),
            country_code: "US".to_string(),
            country_code_3: "USA".to_string(),
            country_name: "United States".to_string(),
            region: "Florida".to_string(),
            city: "Miami".to_string(),
            latitude: 25.7617,
            longitude: -80.1918,
            timezone: "America/New_York".to_string(),
            language: "en".to_string(),
            locale: "en-US".to_string(),
            accept_language: "en-US,en;q=0.9".to_string(),
            currency: "USD".to_string(),
            currency_symbol: "$".to_string(),
            measurement_system: "imperial".to_string(),
            phone_code: "1".to_string(),
            date_format: "MM/DD/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 120,
            plug_type: "A/B".to_string(),
        },
        // ── United Kingdom ──
        LocationProfile {
            name: "London, UK".to_string(),
            country_code: "GB".to_string(),
            country_code_3: "GBR".to_string(),
            country_name: "United Kingdom".to_string(),
            region: "England".to_string(),
            city: "London".to_string(),
            latitude: 51.5074,
            longitude: -0.1278,
            timezone: "Europe/London".to_string(),
            language: "en".to_string(),
            locale: "en-GB".to_string(),
            accept_language: "en-GB,en;q=0.9".to_string(),
            currency: "GBP".to_string(),
            currency_symbol: "£".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "44".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 230,
            plug_type: "G".to_string(),
        },
        // ── Germany ──
        LocationProfile {
            name: "Berlin, Germany".to_string(),
            country_code: "DE".to_string(),
            country_code_3: "DEU".to_string(),
            country_name: "Germany".to_string(),
            region: "Berlin".to_string(),
            city: "Berlin".to_string(),
            latitude: 52.5200,
            longitude: 13.4050,
            timezone: "Europe/Berlin".to_string(),
            language: "de".to_string(),
            locale: "de-DE".to_string(),
            accept_language: "de-DE,de;q=0.9,en;q=0.8".to_string(),
            currency: "EUR".to_string(),
            currency_symbol: "€".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "49".to_string(),
            date_format: "DD.MM.YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "F".to_string(),
        },
        // ── Canada ──
        LocationProfile {
            name: "Toronto, Canada".to_string(),
            country_code: "CA".to_string(),
            country_code_3: "CAN".to_string(),
            country_name: "Canada".to_string(),
            region: "Ontario".to_string(),
            city: "Toronto".to_string(),
            latitude: 43.6532,
            longitude: -79.3832,
            timezone: "America/Toronto".to_string(),
            language: "en".to_string(),
            locale: "en-CA".to_string(),
            accept_language: "en-CA,en;q=0.9,fr;q=0.8".to_string(),
            currency: "CAD".to_string(),
            currency_symbol: "C$".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "1".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 120,
            plug_type: "A/B".to_string(),
        },
        // ── Australia ──
        LocationProfile {
            name: "Sydney, Australia".to_string(),
            country_code: "AU".to_string(),
            country_code_3: "AUS".to_string(),
            country_name: "Australia".to_string(),
            region: "New South Wales".to_string(),
            city: "Sydney".to_string(),
            latitude: -33.8688,
            longitude: 151.2093,
            timezone: "Australia/Sydney".to_string(),
            language: "en".to_string(),
            locale: "en-AU".to_string(),
            accept_language: "en-AU,en;q=0.9".to_string(),
            currency: "AUD".to_string(),
            currency_symbol: "A$".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "61".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 230,
            plug_type: "I".to_string(),
        },
        // ── Japan ──
        LocationProfile {
            name: "Tokyo, Japan".to_string(),
            country_code: "JP".to_string(),
            country_code_3: "JPN".to_string(),
            country_name: "Japan".to_string(),
            region: "Tokyo".to_string(),
            city: "Tokyo".to_string(),
            latitude: 35.6762,
            longitude: 139.6503,
            timezone: "Asia/Tokyo".to_string(),
            language: "ja".to_string(),
            locale: "ja-JP".to_string(),
            accept_language: "ja-JP,ja;q=0.9,en;q=0.8".to_string(),
            currency: "JPY".to_string(),
            currency_symbol: "¥".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "81".to_string(),
            date_format: "YYYY/MM/DD".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 0,
            driving_side: "left".to_string(),
            voltage: 100,
            plug_type: "A/B".to_string(),
        },
        // ── India ──
        LocationProfile {
            name: "Mumbai, India".to_string(),
            country_code: "IN".to_string(),
            country_code_3: "IND".to_string(),
            country_name: "India".to_string(),
            region: "Maharashtra".to_string(),
            city: "Mumbai".to_string(),
            latitude: 19.0760,
            longitude: 72.8777,
            timezone: "Asia/Kolkata".to_string(),
            language: "en".to_string(),
            locale: "en-IN".to_string(),
            accept_language: "en-IN,en;q=0,hi;q=0.8".to_string(),
            currency: "INR".to_string(),
            currency_symbol: "₹".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "91".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "12".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 230,
            plug_type: "C/D/M".to_string(),
        },
        // ── Brazil ──
        LocationProfile {
            name: "São Paulo, Brazil".to_string(),
            country_code: "BR".to_string(),
            country_code_3: "BRA".to_string(),
            country_name: "Brazil".to_string(),
            region: "São Paulo".to_string(),
            city: "São Paulo".to_string(),
            latitude: -23.5505,
            longitude: -46.6333,
            timezone: "America/Sao_Paulo".to_string(),
            language: "pt".to_string(),
            locale: "pt-BR".to_string(),
            accept_language: "pt-BR,pt;q=0.9,en;q=0.8".to_string(),
            currency: "BRL".to_string(),
            currency_symbol: "R$".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "55".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 127,
            plug_type: "C/N".to_string(),
        },
        // ── France ──
        LocationProfile {
            name: "Paris, France".to_string(),
            country_code: "FR".to_string(),
            country_code_3: "FRA".to_string(),
            country_name: "France".to_string(),
            region: "Île-de-France".to_string(),
            city: "Paris".to_string(),
            latitude: 48.8566,
            longitude: 2.3522,
            timezone: "Europe/Paris".to_string(),
            language: "fr".to_string(),
            locale: "fr-FR".to_string(),
            accept_language: "fr-FR,fr;q=0.9,en;q=0.8".to_string(),
            currency: "EUR".to_string(),
            currency_symbol: "€".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "33".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "C/E".to_string(),
        },
        // ── Singapore ──
        LocationProfile {
            name: "Singapore, Singapore".to_string(),
            country_code: "SG".to_string(),
            country_code_3: "SGP".to_string(),
            country_name: "Singapore".to_string(),
            region: "Central".to_string(),
            city: "Singapore".to_string(),
            latitude: 1.3521,
            longitude: 103.8198,
            timezone: "Asia/Singapore".to_string(),
            language: "en".to_string(),
            locale: "en-SG".to_string(),
            accept_language: "en-SG,en;q=0.9,zh;q=0.8,ms;q=0.7".to_string(),
            currency: "SGD".to_string(),
            currency_symbol: "S$".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "65".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 230,
            plug_type: "G".to_string(),
        },
        // ── United Arab Emirates ──
        LocationProfile {
            name: "Dubai, UAE".to_string(),
            country_code: "AE".to_string(),
            country_code_3: "ARE".to_string(),
            country_name: "United Arab Emirates".to_string(),
            region: "Dubai".to_string(),
            city: "Dubai".to_string(),
            latitude: 25.2048,
            longitude: 55.2708,
            timezone: "Asia/Dubai".to_string(),
            language: "ar".to_string(),
            locale: "ar-AE".to_string(),
            accept_language: "ar-AE,ar;q=0.9,en;q=0.8".to_string(),
            currency: "AED".to_string(),
            currency_symbol: "د.إ".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "971".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 220,
            plug_type: "G".to_string(),
        },
        // ── Netherlands ──
        LocationProfile {
            name: "Amsterdam, Netherlands".to_string(),
            country_code: "NL".to_string(),
            country_code_3: "NLD".to_string(),
            country_name: "Netherlands".to_string(),
            region: "North Holland".to_string(),
            city: "Amsterdam".to_string(),
            latitude: 52.3676,
            longitude: 4.9041,
            timezone: "Europe/Amsterdam".to_string(),
            language: "nl".to_string(),
            locale: "nl-NL".to_string(),
            accept_language: "nl-NL,nl;q=0.9,en;q=0.8".to_string(),
            currency: "EUR".to_string(),
            currency_symbol: "€".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "31".to_string(),
            date_format: "DD-MM-YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "C/F".to_string(),
        },
        // ── Switzerland ──
        LocationProfile {
            name: "Zurich, Switzerland".to_string(),
            country_code: "CH".to_string(),
            country_code_3: "CHE".to_string(),
            country_name: "Switzerland".to_string(),
            region: "Zurich".to_string(),
            city: "Zurich".to_string(),
            latitude: 47.3769,
            longitude: 8.5417,
            timezone: "Europe/Zurich".to_string(),
            language: "de".to_string(),
            locale: "de-CH".to_string(),
            accept_language: "de-CH,de;q=0.9,en;q=0.8,fr;q=0.7".to_string(),
            currency: "CHF".to_string(),
            currency_symbol: "CHF".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "41".to_string(),
            date_format: "DD.MM.YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "J".to_string(),
        },
        // ── Sweden ──
        LocationProfile {
            name: "Stockholm, Sweden".to_string(),
            country_code: "SE".to_string(),
            country_code_3: "SWE".to_string(),
            country_name: "Sweden".to_string(),
            region: "Stockholm".to_string(),
            city: "Stockholm".to_string(),
            latitude: 59.3293,
            longitude: 18.0686,
            timezone: "Europe/Stockholm".to_string(),
            language: "sv".to_string(),
            locale: "sv-SE".to_string(),
            accept_language: "sv-SE,sv;q=0.9,en;q=0.8".to_string(),
            currency: "SEK".to_string(),
            currency_symbol: "kr".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "46".to_string(),
            date_format: "YYYY-MM-DD".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "F".to_string(),
        },
        // ── South Korea ──
        LocationProfile {
            name: "Seoul, South Korea".to_string(),
            country_code: "KR".to_string(),
            country_code_3: "KOR".to_string(),
            country_name: "South Korea".to_string(),
            region: "Seoul".to_string(),
            city: "Seoul".to_string(),
            latitude: 37.5665,
            longitude: 126.9780,
            timezone: "Asia/Seoul".to_string(),
            language: "ko".to_string(),
            locale: "ko-KR".to_string(),
            accept_language: "ko-KR,ko;q=0.9,en;q=0.8".to_string(),
            currency: "KRW".to_string(),
            currency_symbol: "₩".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "82".to_string(),
            date_format: "YYYY.MM.DD".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 220,
            plug_type: "F".to_string(),
        },
        // ── Spain ──
        LocationProfile {
            name: "Madrid, Spain".to_string(),
            country_code: "ES".to_string(),
            country_code_3: "ESP".to_string(),
            country_name: "Spain".to_string(),
            region: "Community of Madrid".to_string(),
            city: "Madrid".to_string(),
            latitude: 40.4168,
            longitude: -3.7038,
            timezone: "Europe/Madrid".to_string(),
            language: "es".to_string(),
            locale: "es-ES".to_string(),
            accept_language: "es-ES,es;q=0.9,en;q=0.8".to_string(),
            currency: "EUR".to_string(),
            currency_symbol: "€".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "34".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "C/F".to_string(),
        },
        // ── Italy ──
        LocationProfile {
            name: "Rome, Italy".to_string(),
            country_code: "IT".to_string(),
            country_code_3: "ITA".to_string(),
            country_name: "Italy".to_string(),
            region: "Lazio".to_string(),
            city: "Rome".to_string(),
            latitude: 41.9028,
            longitude: 12.4964,
            timezone: "Europe/Rome".to_string(),
            language: "it".to_string(),
            locale: "it-IT".to_string(),
            accept_language: "it-IT,it;q=0.9,en;q=0.8".to_string(),
            currency: "EUR".to_string(),
            currency_symbol: "€".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "39".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 230,
            plug_type: "C/F/L".to_string(),
        },
        // ── South Africa ──
        LocationProfile {
            name: "Cape Town, South Africa".to_string(),
            country_code: "ZA".to_string(),
            country_code_3: "ZAF".to_string(),
            country_name: "South Africa".to_string(),
            region: "Western Cape".to_string(),
            city: "Cape Town".to_string(),
            latitude: -33.9249,
            longitude: 18.4241,
            timezone: "Africa/Johannesburg".to_string(),
            language: "en".to_string(),
            locale: "en-ZA".to_string(),
            accept_language: "en-ZA,en;q=0.9,af;q=0.8".to_string(),
            currency: "ZAR".to_string(),
            currency_symbol: "R".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "27".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 230,
            plug_type: "C/M/N".to_string(),
        },
        // ── Mexico ──
        LocationProfile {
            name: "Mexico City, Mexico".to_string(),
            country_code: "MX".to_string(),
            country_code_3: "MEX".to_string(),
            country_name: "Mexico".to_string(),
            region: "Mexico City".to_string(),
            city: "Mexico City".to_string(),
            latitude: 19.4326,
            longitude: -99.1332,
            timezone: "America/Mexico_City".to_string(),
            language: "es".to_string(),
            locale: "es-MX".to_string(),
            accept_language: "es-MX,es;q=0.9,en;q=0.8".to_string(),
            currency: "MXN".to_string(),
            currency_symbol: "$".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "52".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 0,
            driving_side: "right".to_string(),
            voltage: 127,
            plug_type: "A/B".to_string(),
        },
        // ── Indonesia ──
        LocationProfile {
            name: "Jakarta, Indonesia".to_string(),
            country_code: "ID".to_string(),
            country_code_3: "IDN".to_string(),
            country_name: "Indonesia".to_string(),
            region: "Jakarta".to_string(),
            city: "Jakarta".to_string(),
            latitude: -6.2088,
            longitude: 106.8456,
            timezone: "Asia/Jakarta".to_string(),
            language: "id".to_string(),
            locale: "id-ID".to_string(),
            accept_language: "id-ID,id;q=0.9,en;q=0.8".to_string(),
            currency: "IDR".to_string(),
            currency_symbol: "Rp".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "62".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 220,
            plug_type: "C/F".to_string(),
        },
        // ── Turkey ──
        LocationProfile {
            name: "Istanbul, Turkey".to_string(),
            country_code: "TR".to_string(),
            country_code_3: "TUR".to_string(),
            country_name: "Turkey".to_string(),
            region: "Istanbul".to_string(),
            city: "Istanbul".to_string(),
            latitude: 41.0082,
            longitude: 28.9784,
            timezone: "Europe/Istanbul".to_string(),
            language: "tr".to_string(),
            locale: "tr-TR".to_string(),
            accept_language: "tr-TR,tr;q=0.9,en;q=0.8".to_string(),
            currency: "TRY".to_string(),
            currency_symbol: "₺".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "90".to_string(),
            date_format: "DD.MM.YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "right".to_string(),
            voltage: 220,
            plug_type: "C/F".to_string(),
        },
        // ── Thailand ──
        LocationProfile {
            name: "Bangkok, Thailand".to_string(),
            country_code: "TH".to_string(),
            country_code_3: "THA".to_string(),
            country_name: "Thailand".to_string(),
            region: "Bangkok".to_string(),
            city: "Bangkok".to_string(),
            latitude: 13.7563,
            longitude: 100.5018,
            timezone: "Asia/Bangkok".to_string(),
            language: "th".to_string(),
            locale: "th-TH".to_string(),
            accept_language: "th-TH,th;q=0.9,en;q=0.8".to_string(),
            currency: "THB".to_string(),
            currency_symbol: "฿".to_string(),
            measurement_system: "metric".to_string(),
            phone_code: "66".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            time_format: "24".to_string(),
            first_day_of_week: 1,
            driving_side: "left".to_string(),
            voltage: 220,
            plug_type: "A/B/C".to_string(),
        },
    ]
}

/// Validate that all location fields are mutually consistent.
/// Returns a list of inconsistency warnings (empty = perfectly consistent).
pub fn validate_consistency(loc: &LocationProfile) -> Vec<String> {
    let mut warnings = Vec::new();

    // Timezone should roughly match country
    let tz_country_prefix = match loc.country_code.as_str() {
        "US" => "America/",
        "GB" => "Europe/",
        "DE" => "Europe/",
        "CA" => "America/",
        "AU" => "Australia/",
        "JP" => "Asia/",
        "IN" => "Asia/",
        "BR" => "America/",
        "FR" => "Europe/",
        "SG" => "Asia/",
        "AE" => "Asia/",
        "NL" => "Europe/",
        "CH" => "Europe/",
        "SE" => "Europe/",
        "KR" => "Asia/",
        "ES" => "Europe/",
        "IT" => "Europe/",
        "ZA" => "Africa/",
        "MX" => "America/",
        "ID" => "Asia/",
        "TR" => "Europe/",
        "TH" => "Asia/",
        _ => "",
    };
    if !tz_country_prefix.is_empty() && !loc.timezone.starts_with(tz_country_prefix) {
        warnings.push(format!(
            "Timezone '{}' does not match country code '{}'",
            loc.timezone, loc.country_code
        ));
    }

    // Locale should start with language
    if !loc.locale.starts_with(&loc.language) {
        warnings.push(format!(
            "Locale '{}' does not start with language '{}'",
            loc.locale, loc.language
        ));
    }

    // Currency should be consistent with country
    let expected_currency = match loc.country_code.as_str() {
        "US" | "MX" => Some("USD"),
        "GB" => Some("GBP"),
        "DE" | "FR" | "NL" | "ES" | "IT" => Some("EUR"),
        "CA" => Some("CAD"),
        "AU" => Some("AUD"),
        "JP" => Some("JPY"),
        "IN" => Some("INR"),
        "BR" => Some("BRL"),
        "SG" => Some("SGD"),
        "AE" => Some("AED"),
        "CH" => Some("CHF"),
        "SE" => Some("SEK"),
        "KR" => Some("KRW"),
        "ZA" => Some("ZAR"),
        "ID" => Some("IDR"),
        "TR" => Some("TRY"),
        "TH" => Some("THB"),
        _ => None,
    };
    if let Some(exp) = expected_currency {
        if loc.currency != exp {
            warnings.push(format!(
                "Currency '{}' does not match expected '{}' for country '{}'",
                loc.currency, exp, loc.country_code
            ));
        }
    }

    warnings
}

/// Global location store — keyed by browser profile ID.
static LOCATION_STORE: Lazy<Mutex<HashMap<String, LocationProfile>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Set the location profile for a browser profile.
pub fn set_location(profile_id: &str, loc: LocationProfile) {
    let mut store = LOCATION_STORE.lock().unwrap();
    store.insert(profile_id.to_string(), loc);
}

/// Get the location profile for a browser profile.
pub fn get_location(profile_id: &str) -> Option<LocationProfile> {
    let store = LOCATION_STORE.lock().unwrap();
    store.get(profile_id).cloned()
}

/// Remove location profile when profile is deleted.
pub fn remove_location(profile_id: &str) {
    let mut store = LOCATION_STORE.lock().unwrap();
    store.remove(profile_id);
}

/// Apply location spoofing to HTTP headers.
pub fn apply_location_to_headers(profile_id: &str, headers: &mut HashMap<String, String>) {
    let loc = match get_location(profile_id) {
        Some(l) => l,
        None => return,
    };

    headers.insert("Accept-Language".to_string(), loc.accept_language);
    headers.insert("CF-IPCountry".to_string(), loc.country_code.clone());
    headers.insert("X-Country".to_string(), loc.country_code.clone());
    headers.insert("X-Locale".to_string(), loc.locale.clone());
    headers.insert("X-Timezone".to_string(), loc.timezone.clone());
    headers.insert("X-Currency".to_string(), loc.currency.clone());
}

/// Generate JavaScript injection snippet that spoofs all location APIs.
/// This is injected into every page before any page script runs.
pub fn generate_location_js(profile_id: &str) -> Option<String> {
    let loc = get_location(profile_id)?;

    let js = format!(
        r#"(function(){{
    // Geolocation API
    Object.defineProperty(navigator, 'geolocation', {{
        configurable: false,
        get: function() {{
            return {{
                getCurrentPosition: function(success, error, opts) {{
                    if (success) {{
                        success({{
                            coords: {{
                                latitude: {lat},
                                longitude: {lon},
                                altitude: null,
                                accuracy: 10,
                                altitudeAccuracy: null,
                                heading: null,
                                speed: null
                            }},
                            timestamp: Date.now()
                        }});
                    }}
                }},
                watchPosition: function(success, error, opts) {{ return 0; }},
                clearWatch: function(id) {{}}
            }};
        }}
    }});

    // Language & Locale
    Object.defineProperty(navigator, 'language', {{
        configurable: false,
        get: function() {{ return '{lang}'; }}
    }});
    Object.defineProperty(navigator, 'languages', {{
        configurable: false,
        get: function() {{ return ['{locale}', '{lang}']; }}
    }});

    // Timezone via Intl
    const origDateTimeFormat = Intl.DateTimeFormat;
    Intl.DateTimeFormat = function(locales, opts) {{
        return new origDateTimeFormat(locales, Object.assign({{ timeZone: '{tz}' }}, opts || {{}}));
    }};
    Intl.DateTimeFormat.prototype = origDateTimeFormat.prototype;
    Intl.DateTimeFormat.supportedLocalesOf = origDateTimeFormat.supportedLocalesOf;

    // Timezone offset
    Date.prototype.getTimezoneOffset = function() {{
        return new Date().toLocaleString('en-US', {{ timeZone: '{tz}' }}).includes('AM') || true;
    }};
    // Better offset calculation
    const tzDate = new Date().toLocaleString('en-US', {{ timeZone: '{tz}' }});
    const localMS = Date.parse(tzDate + ' UTC');
    const utcMS = Date.now();
    const offsetMs = localMS - utcMS;
    Object.defineProperty(Date.prototype, 'getTimezoneOffset', {{
        configurable: false,
        get: function() {{ return -offsetMs / 60000; }}
    }});

    // Temporal API (modern)
    if (typeof Temporal !== 'undefined' && Temporal.Now) {{
        const origZoned = Temporal.Now.zonedDateTimeISO;
        Temporal.Now.zonedDateTimeISO = function() {{
            return origZoned.call(Temporal.Now, '{tz}');
        }};
    }}

    // Currency
    if (typeof Intl !== 'undefined') {{
        const origNumberFormat = Intl.NumberFormat;
        Intl.NumberFormat = function(locales, opts) {{
            opts = opts || {{}};
            if (!opts.currency) opts.currency = '{curr}';
            return new origNumberFormat(locales, opts);
        }};
        Intl.NumberFormat.prototype = origNumberFormat.prototype;
    }}

    console.log('[LocationProfile] Spoofed: {name} ({country_code})');
}})();"#,
        lat = loc.latitude,
        lon = loc.longitude,
        lang = loc.language,
        locale = loc.locale,
        tz = loc.timezone,
        curr = loc.currency,
        name = loc.name,
        country_code = loc.country_code
    );

    Some(js)
}

impl Default for LocationProfile {
    fn default() -> Self {
        // Default: New York, USA
        location_catalog()
            .into_iter()
            .find(|l| l.city == "New York")
            .unwrap_or_else(|| LocationProfile {
                name: "Default".to_string(),
                country_code: "US".to_string(),
                country_code_3: "USA".to_string(),
                country_name: "United States".to_string(),
                region: "New York".to_string(),
                city: "New York".to_string(),
                latitude: 40.7128,
                longitude: -74.0060,
                timezone: "America/New_York".to_string(),
                language: "en".to_string(),
                locale: "en-US".to_string(),
                accept_language: "en-US,en;q=0.9".to_string(),
                currency: "USD".to_string(),
                currency_symbol: "$".to_string(),
                measurement_system: "imperial".to_string(),
                phone_code: "1".to_string(),
                date_format: "MM/DD/YYYY".to_string(),
                time_format: "12".to_string(),
                first_day_of_week: 0,
                driving_side: "right".to_string(),
                voltage: 120,
                plug_type: "A/B".to_string(),
            })
    }
}

pub mod ui;
