# Alert Processor

`alert-processor` es un servicio en Rust que escucha eventos desde Kafka, evalua reglas de alerta asociadas a cada unidad y publica nuevas alertas en otro topic de Kafka.

La idea central es simple:

1. llegan eventos operativos desde Kafka,
2. el servicio identifica que reglas aplican para la unidad del evento,
3. ejecuta la logica de evaluacion por tipo de regla,
4. si alguna regla dispara, publica una alerta nueva.

Esta documentacion esta escrita para personas que no han trabajado antes con procesadores de eventos, Kafka o motores de reglas.

## Que resuelve este servicio

En muchos sistemas operativos o de telemetria, los eventos crudos llegan continuamente: encendido, apagado, ubicacion, cambios de estado, etc. El objetivo de este servicio es convertir esos eventos tecnicos en alertas de negocio.

Ejemplo:

- un evento indica que una unidad reporto `ignition_off`,
- existe una regla activa para esa unidad que escucha ese evento,
- el servicio genera una alerta derivada y la publica para que otros sistemas la consuman.

## Como funciona a alto nivel

```text
PostgreSQL --> carga inicial de reglas --> cache en memoria

Kafka topic de eventos -------------> consumidor de eventos -----+
                                                           |      |
Kafka topic de actualizaciones de reglas -> consumidor ----+      v
                                                        evaluadores por tipo de regla
                                                                  |
                                                                  v
                                                        Kafka topic de alertas
```

Hay dos flujos que corren en paralelo:

- flujo de eventos: procesa eventos de negocio y evalua reglas.
- flujo de actualizaciones de reglas: mantiene la cache en memoria sincronizada sin reiniciar el servicio.

## Componentes principales

### Arranque de la aplicacion

El punto de entrada esta en [src/main.rs](src/main.rs).

Durante el arranque el servicio:

1. inicializa logging,
2. lee configuracion desde variables de entorno,
3. abre conexion a PostgreSQL,
4. carga reglas activas desde base de datos,
5. construye la cache en memoria,
6. crea consumidores y productor de Kafka,
7. inicia el procesador principal.

### Procesador principal

La coordinacion principal esta en [src/app/processor.rs](src/app/processor.rs).

Ese modulo ejecuta dos loops asincronos:

- uno consume eventos del topic operativo,
- otro consume mensajes de actualizacion de reglas.

Cuando llega un evento:

1. lo parsea desde Kafka,
2. obtiene la `unit_id` efectiva,
3. busca reglas aplicables en la cache,
4. localiza el evaluador correcto segun `rule_type`,
5. si el evaluador devuelve una alerta, la publica en Kafka.

### Cache de reglas

La cache esta en [src/cache/rules_cache.rs](src/cache/rules_cache.rs).

Su responsabilidad es mantener en memoria un mapa `unit_id -> reglas aplicables`, para que la evaluacion sea rapida y no dependa de consultar PostgreSQL por cada evento.

Detalles importantes:

- la carga inicial sale de PostgreSQL,
- despues la cache se actualiza con mensajes Kafka de tipo `UPSERT` y `DELETE`,
- si llega una actualizacion vieja, se descarta usando `updated_at`,
- una regla puede estar asociada a multiples unidades.

### Acceso a base de datos

Los modulos [src/db/connection.rs](src/db/connection.rs) y [src/db/repository.rs](src/db/repository.rs) crean el pool y cargan las reglas activas.

La consulta inicial solo trae:

- reglas activas,
- organizaciones con estado `ACTIVE`,
- la lista de `unit_ids` asociada a cada regla.

### Motor de evaluacion

El contrato de evaluacion esta en [src/engine/evaluator.rs](src/engine/evaluator.rs) y el registro de evaluadores en [src/engine/registry.rs](src/engine/registry.rs).

La idea es separar la logica por tipo de regla. Cada `rule_type` apunta a una implementacion concreta.

Evaluadores registrados hoy:

- `ignition_off`: implementado en [src/engine/ignition.rs](src/engine/ignition.rs).
- `geofence`: implementado en [src/engine/geofence.rs](src/engine/geofence.rs).

### Integracion con Kafka

Los adaptadores Kafka viven en:

- [src/kafka/consumer.rs](src/kafka/consumer.rs)
- [src/kafka/producer.rs](src/kafka/producer.rs)

Capacidades relevantes:

- consumidores con SASL configurables,
- soporte para payload JSON normal o JSON serializado como string,
- publicacion de alertas con `unit_id` como key del mensaje.

## Regla actualmente implementada

Hay dos tipos con comportamiento real hoy: `ignition_*` y `geofence`.

Su logica actual es:

- lee `rule.config["event"]`,
- compara ese valor contra `event.event_type`,
- si coinciden sin importar mayusculas/minusculas, genera una alerta.

Eso significa que el nombre del `rule_type` selecciona el evaluador, pero el valor concreto esperado del evento sale de la configuracion de la regla.

### Regla `geofence`

La regla `geofence` usa un `config` con esta forma:

```json
{
    "geofences": ["97003dd7-b9c2-4b76-b681-01842c9c0c7e"],
    "transitions": ["enter", "exit"]
}
```

La logica actual:

- compara `payload.geofence_id` del evento contra `config.geofences`,
- evalua la transicion con `event_type_id`,
- `transitions` acepta alias (`enter`/`exit`) o UUIDs directos de tipo de evento,
- si ambas condiciones coinciden, genera alerta.

## Variables de entorno

El servicio arranca leyendo variables desde el entorno o desde un archivo `.env`. Hay un ejemplo base en [.env.example](.env.example).

Variables requeridas:

- `DATABASE_HOST`
- `DATABASE_NAME`
- `DATABASE_USER`
- `DATABASE_PASSWORD`
- `KAFKA_BROKERS`
- `KAFKA_TOPIC`
- `KAFKA_GROUP_ID`
- `KAFKA_ALERTS_TOPIC`
- `KAFKA_RULES_UPDATES_TOPIC`
- `KAFKA_RULES_UPDATES_GROUP_ID`
- `KAFKA_SASL_USERNAME`
- `KAFKA_SASL_PASSWORD`
- `KAFKA_SASL_MECHANISM`
- `KAFKA_SECURITY_PROTOCOL`

Variables opcionales con valor por defecto:

- `KAFKA_FETCH_WAIT_MAX_MS` = `100`
- `KAFKA_FETCH_MIN_BYTES` = `1`
- `KAFKA_SESSION_TIMEOUT_MS` = `6000`
- `KAFKA_HEARTBEAT_INTERVAL_MS` = `2000`
- `KAFKA_AUTO_OFFSET_RESET` = `latest`

## Ejecucion local

### Opcion 1: ejecutar con Cargo

1. copia el contenido de [.env.example](.env.example) a un archivo `.env` y completa valores reales,
2. asegurate de tener acceso a PostgreSQL y Kafka,
3. ejecuta:

```bash
cargo run
```

### Opcion 2: ejecutar con Docker

Construccion:

```bash
docker build -t alert-processor:local .
```

Ejecucion:

```bash
docker run --rm --env-file .env alert-processor:local
```

El Dockerfile usa build multi-stage con `cargo-chef` para acelerar builds incrementales.

## Despliegue

Existe un workflow en [.github/workflows/deploy.yml](.github/workflows/deploy.yml) que se ejecuta cuando se publica un tag con formato `vX.Y.Z`.

Ese workflow:

1. construye la imagen Docker,
2. la empaqueta en un tarball,
3. copia artefactos y variables de entorno a EC2,
4. carga la imagen en Docker remoto,
5. reinicia el contenedor `alert-processor`.

## Documentacion adicional

- [docs/architecture.md](docs/architecture.md): arquitectura, responsabilidades y recorrido paso a paso.
- [docs/message-contracts.md](docs/message-contracts.md): ejemplos de mensajes de entrada, actualizaciones de reglas y alertas generadas.

## Limitaciones actuales

- no hay persistencia local de estado mas alla de la cache en memoria.
- si una alerta no puede publicarse en Kafka, se registra error en logs pero el proceso continua.

## Recomendacion para nuevos integrantes

Si es tu primera vez en este tipo de sistema, el mejor orden para entenderlo es:

1. leer este README,
2. revisar [docs/architecture.md](docs/architecture.md),
3. mirar [docs/message-contracts.md](docs/message-contracts.md),
4. recorrer despues [src/main.rs](src/main.rs) y [src/app/processor.rs](src/app/processor.rs).
