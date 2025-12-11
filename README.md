# 🚨 zigbee-leak-bot

## Telegram-бот на Rust для уведомлений об утечке воды из Zigbee2MQTT
zigbee-leak-bot — это лёгкий Rust-бот, который подключается к вашему MQTT-брокеру (Mosquitto + Zigbee2MQTT), слушает топики устройств и отправляет уведомления в Telegram при обнаружении:

✔ 💧 утечки воды
✔ 🔋 низкого заряда батареи
✔ 🔧 тампера (вскрытие корпуса)
✔ 📶 изменения качества связи

Бот поддерживает восстановление подключения, фильтрацию по изменениям состояния и удобную структуру сообщений.

## ⚙️ Настройка .env
Открой .env:

```env
TELEGRAM_BOT_TOKEN=123456:ABCDEF....
TELEGRAM_CHAT_ID=123456789       # ваш Telegram numeric ID

MQTT_HOST=192.168.1.109          # IP Raspberry Pi
MQTT_PORT=1883
MQTT_TOPIC=zigbee2mqtt/#         # слушаем все Zigbee-устройства
```

## 🐳 Запуск в Docker (опционально)

```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:stable-slim
WORKDIR /app
COPY --from=builder /app/target/release/zigbee-leak-bot .
COPY .env .env

CMD ["./zigbee-leak-bot"]

```
## Сборка:

```bash
docker build -t zigbee-leak-bot .

```

## Запуск:

```bash
docker run -d --restart=unless-stopped zigbee-leak-bot
