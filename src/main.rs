use eero_client::Client;
use paho_mqtt as mqtt;

#[derive(serde::Deserialize)]
struct AppConfig {
    user_token: String,
    network_id: String,
    period_s: u64,
    device_name: String,
    mqtt_socket_addr: String,
}

#[derive(serde::Serialize)]
struct DeviceConnected {
    device_name: String,
    is_connected: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::builder()
        .add_source(config::File::with_name("config.json"))
        .build()
        .unwrap();
    let config = config.try_deserialize::<AppConfig>().unwrap();
    let mut eero_client = Client::new(&config.user_token);

    let mqtt_client = mqtt::AsyncClient::new(format!("mqtt://{}", config.mqtt_socket_addr))?;
    let connect_opts = mqtt::ConnectOptionsBuilder::new_v5().finalize();
    let _ = mqtt_client.connect(connect_opts).await?;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Received sigint");
                break;
            },
            _ = run(&mut eero_client, &mqtt_client, &config) => {
                println!("Querying for device: {}", config.device_name);
            }
        }
    }

    let _ = mqtt_client.disconnect(None).await?;

    Ok(())
}

async fn run(
    client: &mut Client,
    mqtt_handle: &mqtt::AsyncClient,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, check_result) = tokio::join!(
        tokio::time::sleep(std::time::Duration::from_secs(config.period_s)),
        check_for_device(client, mqtt_handle, config)
    );
    check_result
}

async fn check_for_device(
    client: &mut Client,
    mqtt_handle: &mqtt::AsyncClient,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let devices = client.get_devices_for_network(&config.network_id).await?;
    if let Some(tracked_device) = devices
        .iter()
        .find(|device| device.display_name == config.device_name)
    {
        let msg = mqtt::Message::new(
            "/home/shane",
            serde_json::to_vec(&DeviceConnected {
                device_name: config.device_name.clone(),
                is_connected: tracked_device.connected,
            })?,
            mqtt::QOS_1,
        );
        mqtt_handle.publish(msg).await?;
    } else {
        eprintln!("The configured device is not known to the network");
    }
    Ok(())
}
