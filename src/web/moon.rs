use chrono::prelude::*;
use rust_decimal::prelude::*;

pub struct Moon
{
    qweather_key: String,
}

impl Moon {
    pub fn new(qweather_key: &str) -> Self {
        Self {
            qweather_key: qweather_key.to_string(),
        }
    }

    pub async fn get_phase(&self) -> Result<Decimal, String> {
        let tomorrow = Utc::today().succ();
        let url = format!("https://devapi.qweather.com/v7/astronomy/moon?location=27523&date={}&key={}&lang=en",
                          tomorrow.format("%Y%m%d"),
                          &self.qweather_key
                         );

        let result = reqwest::get(url).await.map_err(|e| e.to_string())?
            .json::<serde_json::value::Value>().await.map_err(|e| e.to_string())?;

        if let serde_json::value::Value::String(val) = result
                .get("moonPhase").ok_or("Missing \"moonPhase\" in server response")?
                .get(0).ok_or("Missing index 0 in \"moonPhase\" array")?
                .get("value").ok_or("Missing \"value\" for \"moonPhase\"")? {
            Ok(Decimal::from_str(val).map_err(|e| e.to_string())?)
        } else {
            Err("Unexpected type of \"moonPhase\"'s \"value\"".to_string())
        }
    }
}
