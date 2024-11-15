use chrono::prelude::*;
use std::sync::Arc;
use std::time::SystemTime;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap::{basic, CborMap};
use crate::web;

pub struct Leds {
    moon: Arc<web::Moon>,
}

impl Leds {
    pub fn new(moon: Arc<web::Moon>) -> Self {
        Self {
            moon,
        }
    }

    async fn get_twilight_pair() -> [SystemTime; 2]
    {
        // TODO: Align it to the time of the year
        // TODO: Reuse with shades?
        let morning_datetime = CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(6, 30, 0).unwrap());
        let evening_datetime = CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(20, 0, 0).unwrap());

        web::Twilight::new().get_pair().await.or::<Result<[SystemTime; 2], String>>(
            Ok([morning_datetime.try_into().unwrap(),
                evening_datetime.try_into().unwrap()]))
            .unwrap()
    }

    async fn get_action_list(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        
        let mut morning_action_list = Vec::new();
        morning_action_list.push(("bbl", (0, 0, 0, 0)));
        morning_action_list.push(("bwl", (0, 0, 0, 0)));
        morning_action_list.push(("drl", (0, 0, 0, 0)));
        morning_action_list.push(("ll", (0, 0, 0, 0)));
        
        // TODO: Twilight time?
        let mut rgbw = (0, 0, 0, 0);
        let moon_phase = self.moon.get_phase().await;
        if let Ok(moon_phase) = moon_phase {
            let factor = 1.0 - ((0.5 - f64::try_from(moon_phase).unwrap()).abs() * 2.0);
            rgbw.0 = (160_f64 * factor).round() as u16;
            rgbw.1 = (180_f64 * factor).round() as u16;
            rgbw.2 = (210_f64 * factor).round() as u16;
        }

        /*
        let mut evening_action_list = Vec::new();
        evening_action_list.push(("bbl", (0,0,0,0)));
        evening_action_list.push(("bwl", (0,0,0,0)));
        evening_action_list.push(("drl", rgbw));
        evening_action_list.push(("ll", (0,0,0,0)));
        */

        let twilight_pair = Self::get_twilight_pair().await;
        let morning_time = twilight_pair[0];
        let evening_time = twilight_pair[1];

        actions.push(Action::new(
            morning_time,
            async move {
                CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::set_led(r, v).await}, None).await
            }
        ));
        actions.push(Action::new(
            evening_time,
            async move {
            }
        ));

        actions
    }

    async fn set_led(rsrc: &str, target: (u16, u16, u16, u16)) -> Result<(), String> {
        let payload = [
                ("r", ciborium::value::Value::Integer(target.0.try_into().unwrap())),
                ("g", ciborium::value::Value::Integer(target.1.try_into().unwrap())),
                ("b", ciborium::value::Value::Integer(target.2.try_into().unwrap())),
                ("w", ciborium::value::Value::Integer(target.3.try_into().unwrap())),
        ];
        let payload = CborMap::from_slice(&payload);

        basic::set_actuator(rsrc, payload).await
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
        ).await;
    }
}
