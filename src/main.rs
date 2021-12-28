use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::prelude::*;
use serde::Deserialize;

use socket2::{Socket, Domain, Type};
use async_coap::prelude::*;
use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use async_coap::uri::Uri;
use futures::prelude::*;

async fn get_next_twilight() -> Result<[SystemTime; 2], String> {
    // TODO: Some kind of cache?
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

async fn wait_until_twilight(twilight_index: usize) -> Result<(), String> {
    let twilight = get_next_twilight().await?;

    let now = SystemTime::now();
    let sleep_time = twilight[twilight_index].duration_since(now).map_err(|e| e.to_string())?;
    tokio::time::sleep(sleep_time).await;
    Ok(())
}

async fn move_shades(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: &str) -> Result<(), String> {
    println!("{} {}", rsrc, target);

    let addr = service_discovery(local_endpoint, rsrc, None).await?;
    send_request(local_endpoint, &addr, rsrc, "val", target).await
}

async fn open_shades_on_dawn(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) {
    println!("Started shades opener");
    loop {
        let result = wait_until_twilight(0).await;

        match result {
            Ok(_) => {
                for rsrc in ["lr", "d1", "d2", "d3"] {
                    // TODO: Add a counter breaking this loop
                    // TODO: Spawn threads to open shades in parallel?
                    loop {
                        let result = move_shades(local_endpoint, rsrc, "up").await;
                        match result {
                            Ok(_) => break,
                            Err(e) => {
                                println!("Error opening shades: {}", e.to_string());
                                tokio::time::sleep(Duration::from_secs(15)).await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Error waiting for dawn: {}", e.to_string());
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
        }
    }
}

async fn close_shades_on_dusk(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) {
    println!("Started shades closer");
    loop {
        let result = wait_until_twilight(1).await;

        match result {
            Ok(_) => {
                for rsrc in ["lr", "d1", "d2", "d3"] {
                    // TODO: Add a counter breaking this loop
                    // TODO: Spawn threads to open shades in parallel?
                    loop {
                        let result = move_shades(local_endpoint, rsrc, "down").await;
                        match result {
                            Ok(_) => break,
                            Err(e) => {
                                println!("Error closing shades: {}", e.to_string());
                                tokio::time::sleep(Duration::from_secs(15)).await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Error waiting for dusk: {}", e.to_string());
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
        }
    }
}

async fn service_discovery(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, srv_name: &str, srv_type: Option<&str>) -> Result<SocketAddr, String> {
    // TODO: Some kind of cache?
    let remote_endpoint = local_endpoint.remote_endpoint_from_uri(uri!("coap://[ff05::1]")).unwrap();

    let future_result= remote_endpoint.send_to(
        rel_ref!("sd"),
        CoapRequest::get()
            .multicast()
            .content_format(ContentFormat::APPLICATION_CBOR)
            .payload_writer(|msg_wrt| {
                let mut payload = BTreeMap::new();
                payload.insert("name", srv_name);
                if let Some(srv_type) = srv_type {
                    payload.insert("type", srv_type);
                }

                msg_wrt.set_msg_code(MsgCode::MethodGet);
                msg_wrt.set_msg_type(MsgType::Non);
                serde_cbor::to_writer(msg_wrt, &payload).unwrap();
                Ok(())
            })
            .use_handler(|context| {
                let data : BTreeMap<String, BTreeMap<String, String>> = serde_cbor::from_slice(context.unwrap().message().payload()).unwrap(); // TODO: Handle errors

                for (service, _details) in data.iter() {
                    if service == srv_name {
                        return Ok(ResponseStatus::Done(context.unwrap().remote_socket_addr()));
                    }
                }

                Ok(ResponseStatus::Continue)
            }) 
        );

    let result = future_result.await;
    result.map_err(|e| e.to_string())
}

async fn send_request(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, addr: &SocketAddr, resource: &str, key: &str, value: &str) -> Result<(), String> {
    let uri = String::new() + "coap://" + &addr.to_string();
    let remote_endpoint = local_endpoint.remote_endpoint_from_uri(Uri::from_str(&uri).unwrap()).unwrap();

    let future_result = remote_endpoint.send_to(
        RelRef::from_str(resource).unwrap(),
        CoapRequest::post()
            .content_format(ContentFormat::APPLICATION_CBOR)
            .payload_writer(|msg_wrt| {
                let mut payload = BTreeMap::new();
                payload.insert(key, value);

                msg_wrt.set_msg_code(MsgCode::MethodPost);
                serde_cbor::to_writer(msg_wrt, &payload);
                Ok(())
            })
            .emit_successful_response()
        );

    let result = future_result.await;
    result.map(|_| ()).map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
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

    let shades_open_local_endpoint = local_endpoint.clone();
    let shades_close_local_endpoint = local_endpoint.clone();

    tasks.push(tokio::spawn(async move {
        open_shades_on_dawn(&shades_open_local_endpoint).await;
    }));
    tasks.push(tokio::spawn(async move {
        close_shades_on_dusk(&shades_close_local_endpoint).await;
    }));

    for task in tasks {
        task.await;
    }
}
