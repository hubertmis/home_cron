use chrono::prelude::*;
use chrono::DateTime;
use rust_decimal::prelude::*;
use std::time::Duration;

#[derive(Debug)]
pub struct Forecast
{
    temperature: Decimal,
    cloudiness: u32,
}

impl Forecast {
    pub fn get_temperature(&self) -> Decimal {
        self.temperature
    }

    pub fn get_cloudiness(&self) -> u32 {
        self.cloudiness
    }
}

pub struct Weather
{
    openweather_key: Option<String>,
    visualcrossing_key: Option<String>,
}

impl Weather {
    pub fn new(openweather_key: Option<String>, visualcrossing_key: Option<String>) -> Self {
        Weather {
            openweather_key,
            visualcrossing_key,
        }
    }

    pub async fn get_temperature_history<Tz>(&self, start_time: DateTime<Tz>, end_time: DateTime<Tz>) -> Result<Vec<Decimal>, String> 
    where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
    {
        let url = format!("https://weather.visualcrossing.com/VisualCrossingWebServices/rest/services/timeline/Krakow,PL/{}/{}?include=hours&elements=datetimeEpoch,temp&unitGroup=metric&key={}",
                          start_time.format("%Y-%m-%d"),
                          end_time.format("%Y-%m-%d"),
                          self.visualcrossing_key.as_ref()
                            .ok_or("Missing visualcrossing key")?
                         );
        let result = reqwest::get(url).await.map_err(|e| e.to_string())?
            .json::<serde_json::value::Value>().await.map_err(|e| e.to_string())?;

        result.get("days").ok_or("Missing days in server response")?
            .as_array().ok_or("Received days ins not an array")?
            .iter()
            .map(|d| d
                .get("hours").ok_or("Missing hours in server response")?
                .as_array().ok_or("Received hours is not an array")?
                .iter()
                .map(|p| {
                    Ok::<(DateTime<Utc>, Decimal), String>((
                        DateTime::from_timestamp(p
                            .get("datetimeEpoch").ok_or(format!("Received data pair {} misses datetimeEpoch", p))?
                            .as_i64().ok_or(format!("Received datetimeEpoch in {} is not integer", p))?,
                        0)
                            .ok_or(format!("Can't parse timestamp received in {}", p))?,
                        p.get("temp").ok_or(format!("Received data pair {} misses temp", p))?
                            .as_f64().ok_or(format!("Received temp in {} is not a number", p))?
                            .try_into().map_err(|e| format!("Can't covert temp in {} to Decimal: {}", p, e))?
                    ))
                })
                .collect::<Result<Vec<(DateTime<Utc>, Decimal)>, String>>()
            )
            .collect::<Result<Vec<Vec<(DateTime<Utc>, Decimal)>>, String>>()?
            .iter()
            .flatten()
            .filter(|p|
                p.0 >= start_time &&
                p.0 < end_time)
            .map(|p| Ok::<Decimal, String>(p.1))
            .collect::<Result<Vec<Decimal>, String>>()
    }

    pub async fn get_forecast(&self, dur: &Duration) -> Result<Forecast, String> {
        let secs_in_3_hours = 3600u64 * 3u64;
        let cnt = (dur.as_secs() + secs_in_3_hours - 1) / secs_in_3_hours;
        let url = format!("https://api.openweathermap.org/data/2.5/forecast?lat=50.061389&lon=19.938333&appid={}&units=metric&cnt={}",
                          self.openweather_key.as_ref()
                            .ok_or("Missing openweather key")?,
                          cnt
                         );
        let result = reqwest::get(url).await.map_err(|e| e.to_string())?
            .json::<serde_json::value::Value>().await.map_err(|e| e.to_string())?;

        let list = result.get("list").ok_or("Missing \"list\" in server response")?
            .as_array().ok_or("\"list\" is not an array")?;

        let mut temp: f64 = 0.0;
        let mut forecast = Forecast {
            cloudiness: 0,
            temperature: Decimal::new(0, 0),
        };
        for item in list {
            if let serde_json::value::Value::Number(temperature) = item
                    .get("main").ok_or("Missing \"main\" entry in one element in the list")?
                    .get("temp").ok_or("Missing \"temp\" for \"main\"")? {
                temp += temperature.as_f64().ok_or("Temperature cannot be converted to f64")?;
            } else {
                return Err("Unexpected type of \"temp\" for \"main\"".to_string());
            }

            if let serde_json::value::Value::Number(cloudiness) = item
                    .get("clouds").ok_or("Missing \"coulds\" entry in one element in the list")?
                    .get("all").ok_or("Missing \"all\" for clouds")? {

                forecast.cloudiness += u32::try_from(cloudiness.as_u64().ok_or("Cloudiness out of range")?)
                    .map_err(|e: std::num::TryFromIntError| e.to_string())?;
            } else {
                return Err("Unexpected type of \"all\" for \"clouds\"".to_string());
            }
        }

        let num_items = u32::try_from(list.len()).map_err(|e: std::num::TryFromIntError| e.to_string())?;

        let temp_f64 = temp / (num_items as f64);
        forecast.temperature = Decimal::from_f64(temp_f64).ok_or(format!("Cannot convert {} to Decimal", temp_f64))?;
        forecast.cloudiness /= num_items;
        Ok(forecast)
    }
}

