mod actuators;
mod coap;
mod state;
mod web;

use std::sync::Arc;

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about=None)]
struct Args {
    #[clap(short, long)]
    openweathermap_token: Option<String>,
    #[clap(short, long)]
    visualcrossing_token: Option<String>,

    #[clap(short, long)]
    qweather_key: String,
}

#[tokio::main]
async fn main() {
    simple_logging::log_to_stderr(log::LevelFilter::Warn);

    let args = Args::parse();

    let moon = web::Moon::new(&args.qweather_key.clone());

    async {
        let result = moon.get_phase().await;
        println!("Moon result: {:?}", result);
    }.await;

    let mut tasks = Vec::new();

    let hvac_state = Arc::new(state::HvacState::new());
    let hvac_state_for_processing = hvac_state.clone();
    let hvac_state_openweathermap_token = args.openweathermap_token.clone();
    let hvac_state_visualcrossing_token = args.visualcrossing_token.clone();

    tasks.push(tokio::spawn(async move {
        let result = hvac_state_for_processing.process(hvac_state_openweathermap_token, hvac_state_visualcrossing_token).await;
        result.unwrap(); // TODO: Any better error handling?
    }));

    let hvac_state_for_shades = hvac_state.clone();
    let shades_openweathermap_token = args.openweathermap_token.clone();
    let shades_visualcrossing_token = args.visualcrossing_token.clone();
    tasks.push(tokio::spawn(async move {
        let weather = Arc::new(web::Weather::new(shades_openweathermap_token, shades_visualcrossing_token));
        let shades = actuators::Shades::new(hvac_state_for_shades, weather);
        shades.process().await;
    }));

    let hvac_state_for_floor_heating = hvac_state.clone();
    tasks.push(tokio::spawn(async move {
        let floor_heating = actuators::FloorHeating::new(hvac_state_for_floor_heating);
        floor_heating.process().await;
    }));

    let hvac_state_for_ac = hvac_state.clone();
    tasks.push(tokio::spawn(async move {
        let ac = actuators::Ac::new(hvac_state_for_ac);
        ac.process().await;
    }));

    tasks.push(tokio::spawn(async move {
        let leds = actuators::Leds::new(Arc::new(moon));
        leds.process().await;
    }));

    /*
    tasks.push(tokio::spawn(async move {
        use chrono::{DateTime, TimeZone, NaiveDateTime, Utc};
        use std::time::Duration;
        let weather = web::Weather::new(&args.openweathermap_token);
        let forecast = weather.get_forecast(&Duration::from_secs(3600*6)).await;
        println!("Forecast: {:?}", forecast);
    }));
    */

    for task in tasks {
        task.await.expect("Failed infinite task");
    }
}
