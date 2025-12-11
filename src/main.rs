use std::{collections::HashMap, env, time::Duration};

use dotenvy::dotenv;
use log::{error, info};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::Deserialize;
use teloxide::{prelude::*, types::ChatId};
use tokio::time::sleep;

#[derive(Debug, Deserialize)]
struct LeakPayload {
    #[serde(default)]
    water_leak: Option<bool>,
    #[serde(default)]
    leak: Option<bool>,
    #[serde(default)]
    battery_low: Option<bool>,
    #[serde(default)]
    battery: Option<u8>,
    #[serde(default)]
    tamper: Option<bool>,
    #[serde(default)]
    linkquality: Option<u16>,
    #[serde(default)]
    voltage: Option<u16>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();

    let bot_token =
        env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN is not set");
    let chat_id: i64 = env::var("TELEGRAM_CHAT_ID")
        .expect("TELEGRAM_CHAT_ID is not set")
        .parse()
        .expect("TELEGRAM_CHAT_ID must be integer");

    let mqtt_host = env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let mqtt_port: u16 = env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".into())
        .parse()
        .unwrap_or(1883);
    let mqtt_topic =
        env::var("MQTT_TOPIC").unwrap_or_else(|_| "zigbee2mqtt/#".into());

    let bot = Bot::new(bot_token);
    let chat = ChatId(chat_id);

    info!("Starting zigbee-leak-bot…");

    run_mqtt_loop(bot, chat, mqtt_host, mqtt_port, mqtt_topic).await;
}

/// Главное MQTT-цикло
async fn run_mqtt_loop(bot: Bot, chat: ChatId, host: String, port: u16, topic: String) {
    // чтобы отправлять алерты только при изменении состояния
    let mut last_states: HashMap<String, bool> = HashMap::new();

    loop {
        let mut mqttoptions = MqttOptions::new("zigbee-leak-bot", host.clone(), port);
        mqttoptions.set_keep_alive(Duration::from_secs(10));

        // AsyncClient + EventLoop (у EventLoop есть poll().await)
        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

        if let Err(e) = client.subscribe(&topic, QoS::AtMostOnce).await {
            error!("MQTT subscribe error: {e:?}");
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        info!("Subscribed to MQTT topic: {topic}");

        // основной цикл
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    if let Ok(payload_str) = String::from_utf8(p.payload.to_vec()) {
                        // игнорируем служебные топики bridge/*
                        if p.topic.starts_with("zigbee2mqtt/bridge") {
                            continue;
                        }

                        let device = extract_device_name(&p.topic);

                        match serde_json::from_str::<LeakPayload>(&payload_str) {
                            Ok(data) => {
                                let leak_flag =
                                    data.water_leak.unwrap_or(false)
                                        || data.leak.unwrap_or(false);

                                let last =
                                    last_states.get(&device).copied().unwrap_or(false);

                                // шлём уведомление только при смене статуса
                                if leak_flag != last {
                                    last_states.insert(device.clone(), leak_flag);

                                    let text =
                                        build_message(&device, &p.topic, &data, leak_flag);

                                    info!("Send alert: {text}");
                                    if let Err(e) =
                                        bot.send_message(chat, text).await
                                    {
                                        error!("Telegram send error: {e:?}");
                                    }
                                }
                            }
                            Err(e) => {
                                error!(
                                    "JSON parse error for topic {}: {e:?}",
                                    p.topic
                                );
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    error!("MQTT error: {e:?}, reconnecting in 5s");
                    sleep(Duration::from_secs(5)).await;
                    break; // выходим из внутреннего цикла, пересоздаём соединение
                }
            }
        }
    }
}

/// Вытащить имя устройства из топика `zigbee2mqtt/<device>`
fn extract_device_name(topic: &str) -> String {
    topic.split('/').nth(1).unwrap_or("unknown").to_string()
}

/// Маппинг device -> красивое имя места
fn pretty_place(device: &str) -> &str {
    match device {
        "Device 1" => "Кухня, под мойкой",
        "leak_kitchen" => "Кухня, под мойкой",
        "leak_bathroom" => "Ванная, возле стиралки",
        _ => device,
    }
}

/// Сборка текста сообщения для Telegram
fn build_message(device: &str, topic: &str, d: &LeakPayload, leak: bool) -> String {
    let place = pretty_place(device);

    let battery = d
        .battery
        .map(|b| format!("{b}%"))
        .unwrap_or_else(|| "?%".to_string());

    let batt_low = d
        .battery_low
        .map(|x| if x { "Да" } else { "Нет" }.to_string())
        .unwrap_or_else(|| "Нет данных".to_string());

    let tamper = d
        .tamper
        .map(|x| {
            if x {
                "⚠️ Датчик трогали/вскрывали".to_string()
            } else {
                "Ок".to_string()
            }
        })
        .unwrap_or_else(|| "Нет данных".to_string());

    let lqi = d
        .linkquality
        .map(|l| l.to_string())
        .unwrap_or_else(|| "?".to_string());

    let voltage = d
        .voltage
        .map(|v| format!("{v} mV"))
        .unwrap_or_else(|| "?".to_string());

    let status = if leak {
        "💧 УТЕЧКА ОБНАРУЖЕНА!"
    } else {
        "✅ Утечка устранена / воды нет"
    };

    format!(
        "{status}\n\
         Место: {place}\n\
         Устройство: {device}\n\
         Топик: {topic}\n\
         \n\
         🔋 Батарея: {battery} (battery_low: {batt_low}, {voltage})\n\
         📶 Связь: {lqi} lqi\n\
         🔧 Тампер: {tamper}"
    )
}
