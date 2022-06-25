use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use chrono::prelude::*;
use rust_decimal::prelude::*;
use std::sync::Arc;
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
    state: tokio::sync::Mutex<Option<HcState>>,
}

impl HvacState {
    pub fn new() -> Self {
        HvacState {
            ext_temp_history: tokio::sync::Mutex::new(Vec::with_capacity(72)),
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

    pub async fn process(&self, local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, token: &str) -> Result<(), String> {
        println!("Starting processing hvac state");
        for n in 0..72 {
            let hours = 72 - n;
            println!("Getting temperature for {} hours ago", hours);
            let temp = web::Weather::new(token).get_temperature_history(Utc::now() - chrono::Duration::hours(hours)).await;
            println!("Temp: {:?}", temp);
            self.ext_temp_history.lock().await.push(temp.unwrap());
        }
	
        let mut last_measurement_time = Utc::now() - chrono::Duration::hours(1);

        loop {
            // TODO: Some retries, trying other sources?
            let curr_val = coap::Weather::new(local_endpoint.clone()).get_temperature().await;
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

            self.update_state().await;

            // Wait one more hour
            last_measurement_time = last_measurement_time + chrono::Duration::hours(1);
            let next_measurement_time: SystemTime = (last_measurement_time + chrono::Duration::hours(1)).try_into().unwrap();

            let sleep_time = next_measurement_time.duration_since(SystemTime::now()).map_err(|e| e.to_string())?;
            tokio::time::sleep(sleep_time).await;
        }
    }

    pub async fn average(&self) -> Decimal {
        let vec = self.ext_temp_history.lock().await;
        let mut sum = Decimal::new(0, 0);
        for val in vec.iter() {
            sum += val;
        }
        let avg = sum / Decimal::new(vec.len().try_into().unwrap(), 0);

        avg
    }
}

