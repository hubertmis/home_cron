mod actuators;
mod coap;
mod state;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;

use socket2::{Socket, Domain, Type};
use async_coap::prelude::*;
use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use futures::prelude::*;
use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about=None)]
struct Args {
    #[clap(short, long)]
    openweathermap_token: String,
}

#[tokio::main]
async fn main() {
    simple_logging::log_to_stderr(log::LevelFilter::Warn);

    let args = Args::parse();

    let udp_socket = Socket::new(Domain::IPV6, Type::DGRAM, None).expect("Socket creating failed");
    let address: SocketAddr = "[::]:0".parse().unwrap();
    let address = address.into();
    udp_socket.set_nonblocking(true).unwrap();
    udp_socket.set_multicast_hops_v6(16).expect("Setting multicast hops failed");
    udp_socket.bind(&address).expect("UDP bind failed");

    let socket = AllowStdUdpSocket::from_std(udp_socket.into());
    let local_endpoint = Arc::new(DatagramLocalEndpoint::new(socket));

    let mut tasks = Vec::new();

    tasks.push(tokio::spawn(local_endpoint
                            .clone()
                            .receive_loop_arc(null_receiver!())
                            .map(|err| panic!("CoAP recv loop terminated: {}", err))
    ));

    let hvac_state = Arc::new(state::HvacState::new());
    let hvac_state_local_endpoint = local_endpoint.clone();
    let hvac_state_for_processing = hvac_state.clone();
    let hvac_state_openweathermap_token = args.openweathermap_token.clone();

    tasks.push(tokio::spawn(async move {
        let result = hvac_state_for_processing.process(hvac_state_local_endpoint, &hvac_state_openweathermap_token).await;
        result.unwrap(); // TODO: Any better error handling?
    }));

    let shades_endpoint = local_endpoint.clone();
    let hvac_state_for_shades = hvac_state.clone();
    let shades_openweathermap_token = args.openweathermap_token.clone();
    tasks.push(tokio::spawn(async move {
        let weather = Arc::new(web::Weather::new(&shades_openweathermap_token));
        let shades = actuators::Shades::new(shades_endpoint, hvac_state_for_shades, weather);
        shades.process().await;
    }));

    let floor_heating_endpoint = local_endpoint.clone();
    let hvac_state_for_floor_heating = hvac_state.clone();
    tasks.push(tokio::spawn(async move {
        let floor_heating = actuators::FloorHeating::new(floor_heating_endpoint, hvac_state_for_floor_heating);
        floor_heating.process().await;
    }));

    let ac_endpoint = local_endpoint.clone();
    let hvac_state_for_ac = hvac_state.clone();
    tasks.push(tokio::spawn(async move {
        let ac = actuators::Ac::new(ac_endpoint, hvac_state_for_ac);
        ac.process().await;
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
