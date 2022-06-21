use chrono::prelude::*;
use rust_decimal::prelude::*;
use std::time::Duration;

#[derive(Debug)]
pub struct Forecast
{
    cloudiness: u32,
}

impl Forecast {
    pub fn get_cloudiness(&self) -> u32 {
        self.cloudiness
    }
}

pub struct Weather
{
    openweather_key: String,
}

impl Weather {
    pub fn new(openweather_key: &str) -> Self {
        Weather {
            openweather_key: openweather_key.to_string(),
        }
    }

    pub async fn get_temperature_history<Tz: TimeZone>(&self, time: DateTime<Tz>) -> Result<Decimal, String> {
        let url = format!("https://api.openweathermap.org/data/2.5/onecall/timemachine?lat=50.061389&lon=19.938333&dt={}&appid={}&units=metric",
                          time.timestamp(),
                          &self.openweather_key
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

    pub async fn get_forecast(&self, dur: &Duration) -> Result<Forecast, String> {
        let secs_in_3_hours = 3600u64 * 3u64;
        let cnt = (dur.as_secs() + secs_in_3_hours - 1) / secs_in_3_hours;
        let url = format!("https://api.openweathermap.org/data/2.5/forecast?lat=50.061389&lon=19.938333&appid={}&units=metric&cnt={}",
                          self.openweather_key,
                          cnt
                         );
        let result = reqwest::get(url).await.map_err(|e| e.to_string())?
            .json::<serde_json::value::Value>().await.map_err(|e| e.to_string())?;

        let list = result.get("list").ok_or("Missing \"list\" in server response")?
            .as_array().ok_or("\"list\" is not an array")?;

        let mut forecast = Forecast {
            cloudiness: 0,
        };
        for item in list {
            if let serde_json::value::Value::Number(cloudiness) = item
                    .get("clouds").ok_or("Missing \"coulds\" entry in one element in the list")?
                    .get("all").ok_or("Missing \"all\" for clouds")? {

                forecast.cloudiness += u32::try_from(cloudiness.as_u64().ok_or("Cloudiness out of range")?)
                    .map_err(|e: std::num::TryFromIntError| e.to_string())?;
            } else {
                return Err("Unexpected type of \"all\" for \"clouds\"".to_string());
            }
        }

        forecast.cloudiness /= u32::try_from(list.len()).map_err(|e: std::num::TryFromIntError| e.to_string())?;
        Ok(forecast)
    }
}

