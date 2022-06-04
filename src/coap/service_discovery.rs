use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use async_coap::prelude::*;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct ServiceDiscovery {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
    // TODO: Some cache?
}

impl ServiceDiscovery {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) -> Self {
        ServiceDiscovery {
            local_endpoint
        }
    }

    pub async fn service_discovery(&self, srv_name: &str, srv_type: Option<&str>) -> Result<SocketAddr, String> {
        let remote_endpoint = self.local_endpoint.remote_endpoint_from_uri(uri!("coap://[ff05::1]")).unwrap();

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
                    ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
                    Ok(())
                })
                .use_handler(|context| {
                    let data : BTreeMap<String, BTreeMap<String, String>> = ciborium::de::from_reader(context.unwrap().message().payload()).unwrap(); // TODO: Handle errors

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
}
