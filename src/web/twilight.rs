use chrono::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::SystemTime;


// TODO: Some kind of cache?
pub struct Twilight();

impl Twilight {
    pub fn new() -> Self {
        Twilight()
    }

    pub async fn get_pair(&self) -> Result<[SystemTime; 2], String> {
        #[derive(Deserialize)]
        struct SunData {
            results: BTreeMap<String, String>,
            status: String,
        }

        async fn sun_time_get<Tz: TimeZone>(day: Date<Tz>) -> Result<SunData, reqwest::Error> 
        where Tz::Offset: core::fmt::Display
        {
            let result = reqwest::get(format!("https://api.sunrise-sunset.org/json?lat=50.061389&lng=19.938333&date={}",
                                              &day.format("%Y-%m-%d").to_string()
                                             )).await?
                         .json::<SunData>().await?;
            Ok(result)
        }

        let today = Utc::today();
        let tomorrow = today.succ();

        let sun_data_today = sun_time_get(today).await.map_err(|e| e.to_string())?;
        let sun_data_tomorrow = sun_time_get(tomorrow).await.map_err(|e| e.to_string())?;

        if sun_data_today.status != "OK".to_string() {
            return Err("Status of retrieved today sun data is not OK".to_string());
        }
        if sun_data_tomorrow.status != "OK".to_string() {
            return Err("Status of retrieved tomorrow sun data is not OK".to_string());
        }

        if let (Some(twilight_begin_today_str), Some(twilight_end_today_str), 
                Some(twilight_begin_tomorrow_str), Some(twilight_end_tomorrow_str)) = 
                (sun_data_today.results.get(&"civil_twilight_begin".to_string()),
                 sun_data_today.results.get(&"civil_twilight_end".to_string()),
                 sun_data_tomorrow.results.get(&"civil_twilight_begin".to_string()),
                 sun_data_tomorrow.results.get(&"civil_twilight_end".to_string())) {
            let twilight_beg_today_time = NaiveTime::parse_from_str(twilight_begin_today_str, "%r").map_err(|e| e.to_string())?;
            let twilight_end_today_time = NaiveTime::parse_from_str(twilight_end_today_str, "%r").map_err(|e| e.to_string())?;
            let twilight_beg_tomorrow_time = NaiveTime::parse_from_str(twilight_begin_tomorrow_str, "%r").map_err(|e| e.to_string())?;
            let twilight_end_tomorrow_time = NaiveTime::parse_from_str(twilight_end_tomorrow_str, "%r").map_err(|e| e.to_string())?;

            let now = Utc::now();
            let twilight_begin_today = today.and_time(twilight_beg_today_time).unwrap();
            let twilight_end_today = today.and_time(twilight_end_today_time).unwrap();
            let twilight_begin_tomorrow = tomorrow.and_time(twilight_beg_tomorrow_time).unwrap();
            let twilight_end_tomorrow = tomorrow.and_time(twilight_end_tomorrow_time).unwrap();

            let twilight_begin = if now > twilight_begin_today { twilight_begin_tomorrow } else { twilight_begin_today };
            let twilight_end = if now > twilight_end_today { twilight_end_tomorrow } else {twilight_end_today };

            Ok([twilight_begin.try_into().unwrap(), twilight_end.try_into().unwrap()])
        } else {
            Err("Missing twilight time in retrieved sun data".to_string())
        }
    }
}
