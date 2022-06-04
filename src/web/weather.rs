use chrono::prelude::*;
use rust_decimal::prelude::*;

pub struct Weather();

impl Weather {
    pub fn new() -> Self {
        Weather()
    }

    pub async fn get_temperature_history<Tz: TimeZone>(&self, openweather_key: &str, time: DateTime<Tz>) -> Result<Decimal, String> {
        let url = format!("https://api.openweathermap.org/data/2.5/onecall/timemachine?lat=50.061389&lon=19.938333&dt={}&appid={}&units=metric",
                          time.timestamp(),
                          openweather_key 
                         );
        let result = reqwest::get(url).await.map_err(|e| e.to_string())?
            .json::<serde_json::value::Value>().await.map_err(|e| e.to_string())?;

        if let serde_json::value::Value::Number(val) = result
                .get("current").ok_or("Missing \"current\" in server response")?
                .get("temp").ok_or("Missing \"current.temp\" in server response")? {
            Ok(val.as_f64().ok_or("Temperature out of range")?.try_into().map_err(|e: rust_decimal::Error| e.to_string())?)
        } else {
            Err("Unexpected type of temperature".to_string())
        }
    }
}

