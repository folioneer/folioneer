use crate::context::currency::domain::rate_provider::{EurSnapshot, RateProvider};
use crate::context::currency::domain::CurrencyRateSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::time::Duration;

const ECB_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
const REQUEST_TIMEOUT_SECS: u64 = 10;
const MICROS_PER_UNIT: f64 = 1_000_000.0;

/// Production [`RateProvider`] backed by the ECB XML daily feed (ADR-009, FXR-070).
pub struct ReqwestEcbClient {
    client: reqwest::Client,
}

impl ReqwestEcbClient {
    /// Creates a new client with a 10-second per-request timeout. Fails only if
    /// the TLS backend cannot be initialised — an unrecoverable environment
    /// fault surfaced at startup rather than panicked on.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("building the ECB HTTP client")?;
        Ok(Self { client })
    }
}

#[async_trait]
impl RateProvider for ReqwestEcbClient {
    async fn fetch_eur_snapshot(&self) -> Result<EurSnapshot> {
        let response = self
            .client
            .get(ECB_URL)
            .send()
            .await
            .context("ECB fetch request failed")?;
        if !response.status().is_success() {
            anyhow::bail!("ECB returned status {}", response.status());
        }
        let body = crate::shared::infrastructure::http::read_capped_text(response)
            .await
            .context("ECB response read failed")?;
        parse_ecb_snapshot(&body)
    }
}

/// Parses the ECB XML daily feed body into an `EurSnapshot` (FXR-070).
///
/// The feed uses a gesmes:Envelope with nested `<Cube time='YYYY-MM-DD'>` and per-currency
/// `<Cube currency='XXX' rate='1.234'/>` elements. Rates are converted to i64 micros.
/// EUR itself is NOT present in the returned map (it is the implicit base). A body with no
/// dated `Cube` is treated as malformed.
pub(crate) fn parse_ecb_snapshot(body: &str) -> Result<EurSnapshot> {
    let mut reader = Reader::from_str(body);
    let mut date: Option<String> = None;
    let mut rates: HashMap<String, i64> = HashMap::new();

    loop {
        match reader.read_event().context("ECB XML parse failed")? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element) if element.name().as_ref() == b"Cube" => {
                let mut currency: Option<String> = None;
                let mut rate: Option<f64> = None;
                for attribute in element.attributes() {
                    let attribute = attribute.context("ECB XML attribute parse failed")?;
                    match attribute.key.as_ref() {
                        b"time" => {
                            date = Some(
                                attribute
                                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .context("ECB time attribute decode failed")?
                                    .into_owned(),
                            );
                        }
                        b"currency" => {
                            currency = Some(
                                attribute
                                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .context("ECB currency attribute decode failed")?
                                    .into_owned(),
                            );
                        }
                        b"rate" => {
                            let raw = attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .context("ECB rate attribute decode failed")?;
                            let value = raw.parse::<f64>().context("ECB rate parse failed")?;
                            // Reject non-finite/non-positive values (e.g. "inf"/"nan",
                            // which parse to f64) before the micros cast.
                            if !value.is_finite() || value <= 0.0 {
                                anyhow::bail!("ECB rate value out of range: {raw}");
                            }
                            rate = Some(value);
                        }
                        _ => {}
                    }
                }
                if let (Some(currency), Some(rate)) = (currency, rate) {
                    rates.insert(currency, (rate * MICROS_PER_UNIT).round() as i64);
                }
            }
            _ => {}
        }
    }

    let date = date.context("ECB feed contained no dated Cube element")?;
    Ok(EurSnapshot {
        date,
        rates,
        source: CurrencyRateSource::Ecb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::currency::domain::CurrencyRateSource;

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
 <gesmes:subject>Reference rates</gesmes:subject>
 <gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender>
 <Cube><Cube time='2026-06-01'>
   <Cube currency='USD' rate='1.1646'/>
   <Cube currency='GBP' rate='0.86493'/>
   <Cube currency='JPY' rate='185.74'/>
 </Cube></Cube>
</gesmes:Envelope>"#;

    // The constructor builds a reqwest client successfully in a normal environment.
    #[test]
    fn new_builds_a_client() {
        assert!(ReqwestEcbClient::new().is_ok());
    }

    // FXR-070 — parses date from the `time` attribute and sets source = Ecb
    #[test]
    fn parse_ecb_snapshot_parses_date_and_source() {
        let snapshot = parse_ecb_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.date, "2026-06-01");
        assert_eq!(snapshot.source, CurrencyRateSource::Ecb);
    }

    // FXR-070 — USD rate is converted to 1_164_600 micros
    #[test]
    fn parse_ecb_snapshot_usd_rate_to_micros() {
        let snapshot = parse_ecb_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.rates.get("USD").copied(), Some(1_164_600));
    }

    // FXR-070 — GBP rate is converted to 864_930 micros
    #[test]
    fn parse_ecb_snapshot_gbp_rate_to_micros() {
        let snapshot = parse_ecb_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.rates.get("GBP").copied(), Some(864_930));
    }

    // FXR-070 — JPY rate is converted to 185_740_000 micros
    #[test]
    fn parse_ecb_snapshot_jpy_rate_to_micros() {
        let snapshot = parse_ecb_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.rates.get("JPY").copied(), Some(185_740_000));
    }

    // FXR-070 — malformed XML body → Err
    #[test]
    fn parse_ecb_snapshot_malformed_xml_returns_err() {
        let result = parse_ecb_snapshot("<not valid xml >>>>");
        assert!(result.is_err(), "malformed XML must return Err");
    }

    // Hardening — `str::parse::<f64>` accepts "inf"/"nan"; reject them before the cast.
    #[test]
    fn parse_ecb_snapshot_rejects_non_finite_rate() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
 <Cube><Cube time='2026-06-01'><Cube currency='USD' rate='inf'/></Cube></Cube>
</gesmes:Envelope>"#;
        assert!(
            parse_ecb_snapshot(body).is_err(),
            "non-finite rate must return Err"
        );
    }

    // Hardening — a negative rate from an anomalous feed is rejected.
    #[test]
    fn parse_ecb_snapshot_rejects_negative_rate() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
 <Cube><Cube time='2026-06-01'><Cube currency='USD' rate='-1.5'/></Cube></Cube>
</gesmes:Envelope>"#;
        assert!(
            parse_ecb_snapshot(body).is_err(),
            "negative rate must return Err"
        );
    }
}
