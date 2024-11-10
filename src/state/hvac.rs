use chrono::prelude::*;
use rust_decimal::prelude::*;
use std::time::{Duration, SystemTime};

use crate::coap;
use crate::web;

#[derive(Copy, Clone)]
pub enum HcState {
    HeatingActive,
    HeatingPassive,
    CoolingPassive,
    CoolingActive,
}

pub struct HvacState {
    ext_temp_history: tokio::sync::Mutex<Vec<Decimal>>,
    ext_temp_forecast: tokio::sync::Mutex<Option<Decimal>>,
    state: tokio::sync::Mutex<Option<HcState>>,
}

impl HvacState {
    pub fn new() -> Self {
        HvacState {
            ext_temp_history: tokio::sync::Mutex::new(Vec::with_capacity(72)),
            ext_temp_forecast: tokio::sync::Mutex::new(None),
            state: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn get_state(&self) -> HcState {
        loop {
            let state = *self.state.lock().await;
            if state.is_some() {
                return state.unwrap();
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn update_state(&self) {
        let avg = self.average().await;
        println!("Avg: {}", avg);
        let prev_state = *self.state.lock().await;

        *self.state.lock().await = match prev_state {
            None => {
                if avg > Decimal::new(18, 0) {
                    Some(HcState::CoolingActive)
                } else if avg > Decimal::new(13, 0) {
                    Some(HcState::CoolingPassive)
                } else if avg > Decimal::new(11, 0) {
                    Some(HcState::HeatingPassive)
                } else {
                    Some(HcState::HeatingActive)
                }
            },
            Some(HcState::HeatingActive) => {
                if avg > Decimal::new(20, 0) {
                    Some(HcState::CoolingActive)
                } else if avg > Decimal::new(15, 0) {
                    Some(HcState::CoolingPassive)
                } else if avg > Decimal::new(13, 0) {
                    Some(HcState::HeatingPassive)
                } else {
                    Some(HcState::HeatingActive)
                }
            },
            Some(HcState::HeatingPassive) => {
                if avg > Decimal::new(20, 0) {
                    Some(HcState::CoolingActive)
                } else if avg > Decimal::new(15, 0) {
                    Some(HcState::CoolingPassive)
                } else if avg > Decimal::new(11, 0) {
                    Some(HcState::HeatingPassive)
                } else {
                    Some(HcState::HeatingActive)
                }
            },
            Some(HcState::CoolingPassive) => {
                if avg > Decimal::new(20, 0) {
                    Some(HcState::CoolingActive)
                } else if avg > Decimal::new(13, 0) {
                    Some(HcState::CoolingPassive)
                } else if avg > Decimal::new(11, 0) {
                    Some(HcState::HeatingPassive)
                } else {
                    Some(HcState::HeatingActive)
                }
            },
            Some(HcState::CoolingActive) => {
                if avg > Decimal::new(18, 0) {
                    Some(HcState::CoolingActive)
                } else if avg > Decimal::new(13, 0) {
                    Some(HcState::CoolingPassive)
                } else if avg > Decimal::new(11, 0) {
                    Some(HcState::HeatingPassive)
                } else {
                    Some(HcState::HeatingActive)
                }
            },
        }
    }

    pub async fn process(&self, openweather_token: Option<String>, visualcrossing_token: Option<String>) -> Result<(), String> {
        println!("Starting processing hvac state");
        let weather = web::Weather::new(openweather_token, visualcrossing_token);

        let now = Utc::now();
        let start_time = now - chrono::Duration::hours(72);
        println!("Getting temperature for range 72 hours ago until now");
        let temps = weather.get_temperature_history(start_time, now).await.unwrap();
        for temp in &temps {
            println!("Temp: {:?}", temp);
        }
        self.ext_temp_history.lock().await.extend_from_slice(&temps);
	
        let mut last_measurement_time = Utc::now() - chrono::Duration::hours(1);

        loop {
            // TODO: Some retries, trying other sources?
            let curr_val = coap::Weather::new().get_temperature().await;
            if let Ok(curr_val) = curr_val {
                async {
                    let mut temp_history = self.ext_temp_history.lock().await;
                    temp_history.remove(0);
                    temp_history.push(curr_val);
                    println!("Temp: {:?}", curr_val);
                }.await;
            } else {
                // Could not get temperature. Copy last one as fallback solution
                async {
                    let mut temp_history = self.ext_temp_history.lock().await;
                    let last = temp_history.last().cloned();
                    if let Some(last) = last {
                        temp_history.remove(0);
                        temp_history.push(last);
                        println!("Guessing temp: {:?}", last);
                    }
                }.await;
            }

            println!("Getting temperature forecast");
            let forecast = weather.get_forecast(&Duration::from_secs(24 * 3600)).await;
            async {
                let mut temp_forecast = self.ext_temp_forecast.lock().await;
                if let Ok(forecast) = forecast {
                    *temp_forecast = Some(forecast.get_temperature());
                    println!("Temp: {:?}", *temp_forecast);
                } else {
                    *temp_forecast = None;
                }
            }.await;

            self.update_state().await;

            // Wait one more hour
            last_measurement_time = last_measurement_time + chrono::Duration::hours(1);
            let next_measurement_time: SystemTime = (last_measurement_time + chrono::Duration::hours(1)).try_into().unwrap();

            let sleep_time = next_measurement_time.duration_since(SystemTime::now()).map_err(|e| e.to_string())?;
            tokio::time::sleep(sleep_time).await;
        }
    }

    async fn past_average(&self) -> Decimal {
        let vec = self.ext_temp_history.lock().await;
        let mut sum = Decimal::new(0, 0);
        for val in vec.iter() {
            sum += val;
        }
        let avg = sum / Decimal::new(vec.len().try_into().unwrap(), 0);

        avg
    }

    pub async fn average(&self) -> Decimal {
        let past_avg = self.past_average().await;
        let forecast = self.ext_temp_forecast.lock().await;

        if let Some(future_avg) = *forecast {
            let sum = past_avg + future_avg;
            let avg = sum / Decimal::new(2, 0);

            avg
        } else {
            past_avg
        }
    }
}

