# Contratos de mensajes

Este documento resume que entra y que sale del servicio. No pretende ser una especificacion formal completa, sino una guia practica para entender el intercambio de mensajes.

## 1. Evento de entrada

El consumidor de eventos espera un mensaje que pueda deserializarse como `IncomingEvent` definido en [src/domain/event.rs](../src/domain/event.rs).

Campos principales:

- `event_id` (o `id`): identificador unico del evento.
- `organization_id`: organizacion asociada, opcional.
- `unit_id`: unidad asociada, opcional.
- `event_type`: tipo de evento de negocio (ejemplo: ignition).
- `event_type_id`: identificador del tipo de evento (ejemplo: geofence enter/exit).
- `occurred_at`: cuando ocurrio realmente.
- `received_at`: cuando fue recibido por el sistema previo (opcional en algunos emisores).
- `payload`: cuerpo adicional flexible en JSON.

Ejemplo:

```json
{
  "event_id": "0ef2f6a4-9a32-48da-b394-0bd0c81df0c2",
  "organization_id": "d7e11a4b-017b-4799-bf66-77f0eab0f91d",
  "unit_id": "0c2d17d2-1968-4e58-95c0-c5539ae196fd",
  "schema_version": 1,
  "event_type": "ignition_off",
  "source": {
    "type": "telematics",
    "id": "provider-a",
    "message_id": "18f43fc2-0c74-40bf-aef2-cf23332a2c14"
  },
  "unit": {
    "id": "0c2d17d2-1968-4e58-95c0-c5539ae196fd"
  },
  "source_epoch": 1712496000,
  "occurred_at": "2026-04-07T14:20:00Z",
  "received_at": "2026-04-07T14:20:03Z",
  "payload": {
    "engine": "off",
    "speed": 0
  }
}
```

### Nota sobre `unit_id`

El servicio usa `event.unit_id` si viene presente. Si no viene, usa `event.unit.id`.

### Formatos de payload aceptados en Kafka

El consumidor soporta dos formatos:

1. JSON normal.
2. JSON serializado como string dentro de otro JSON.

Ejemplo del segundo caso:

```json
"{\"event_id\":\"0ef2f6a4-9a32-48da-b394-0bd0c81df0c2\",\"event_type\":\"ignition_off\"}"
```

## 2. Mensajes de actualizacion de reglas

La cache de reglas se actualiza con mensajes JSON en el topic configurado por `KAFKA_RULES_UPDATES_TOPIC`.

Formato general:

- `operation = "UPSERT"` para crear o actualizar.
- `operation = "DELETE"` para eliminar.
- En `UPSERT`, `rule.name` se usa para poblar `alert_name` en la salida.
- En `UPSERT`, `rule.context.units` puede incluir metadatos de unidad (`id`, `name`) para poblar `unit_name` en la salida.

### Ejemplo de UPSERT

```json
{
  "operation": "UPSERT",
  "rule": {
    "id": "417d8f6d-081d-4022-9d1a-b92b3fd3b851",
    "organization_id": "d7e11a4b-017b-4799-bf66-77f0eab0f91d",
    "name": "Motor apagado",
    "type": "ignition_off",
    "config": {
      "event": "ignition_off"
    },
    "unit_ids": [
      "0c2d17d2-1968-4e58-95c0-c5539ae196fd"
    ],
    "context": {
      "units": [
        {
          "id": "0c2d17d2-1968-4e58-95c0-c5539ae196fd",
          "name": "Camioneta Juan"
        }
      ]
    },
    "is_active": true,
    "updated_at": "2026-04-07T14:00:00Z"
  }
}
```

### Ejemplo de DELETE

```json
{
  "operation": "DELETE",
  "rule_id": "417d8f6d-081d-4022-9d1a-b92b3fd3b851",
  "updated_at": "2026-04-07T15:00:00Z"
}
```

### Regla importante de versionado

Si llega un update con `updated_at` mas viejo que la version actual en cache, el servicio lo ignora para evitar pisar datos nuevos con datos stale.

## 3. Regla `ignition_off`

Hoy la implementacion funcional depende de:

- `rule.type` o `rule_type`: debe apuntar al evaluador `ignition_off`.
- `rule.name`: se propaga como `alert_name` en el mensaje de salida.
- `rule.config.event`: debe contener el nombre del evento esperado.

Ejemplo de configuracion minima:

```json
{
  "event": "ignition_off"
}
```

Si el `event_type` del mensaje coincide con ese valor, se genera alerta.

## 4. Alerta generada

Cuando una regla dispara, el servicio publica un mensaje con la estructura `Alert` definida en [src/domain/alert.rs](../src/domain/alert.rs).

Campos principales:

- `id`: identificador nuevo de la alerta.
- `organization_id`: organizacion asociada.
- `unit_id`: unidad asociada.
- `rule_id`: regla que disparo.
- `source_type`: hoy se fija en `event`.
- `source_id`: contiene el `event_id` original.
- `alert_type`: toma `event_type` cuando existe; si no, usa `event_type_id`.
- `alert_name`: contiene el `name` de la regla que disparo.
- `unit_name`: nombre de la unidad, cuando esta disponible en la metadata de reglas.
- `payload`: reutiliza el payload del evento.
- `occurred_at`: conserva la fecha del evento original.

Ejemplo:

```json
{
  "id": "4f7db7cf-f8bc-4d9b-bf31-0efff4ac77dd",
  "organization_id": "d7e11a4b-017b-4799-bf66-77f0eab0f91d",
  "unit_id": "0c2d17d2-1968-4e58-95c0-c5539ae196fd",
  "unit_name": "Camioneta Juan",
  "rule_id": "417d8f6d-081d-4022-9d1a-b92b3fd3b851",
  "source_type": "event",
  "source_id": "0ef2f6a4-9a32-48da-b394-0bd0c81df0c2",
  "alert_type": "ignition_off",
  "alert_name": "Motor apagado",
  "payload": {
    "engine": "off",
    "speed": 0
  },
  "occurred_at": "2026-04-07T14:20:00Z"
}
```

## 5. Topic y key de salida

Las alertas se publican en el topic configurado por `KAFKA_ALERTS_TOPIC`.

La key Kafka del mensaje es `unit_id`. Eso suele ser util para particionar o mantener afinidad por unidad en consumidores posteriores.

## 6. Regla `geofence`

Las reglas de geocerca usan el mismo formato general de `alert_rules`, con un `config` distinto.

Configuracion esperada:

```json
{
  "geofences": [
    "97003dd7-b9c2-4b76-b681-01842c9c0c7e"
  ],
  "transitions": [
    "enter",
    "exit"
  ]
}
```

Semantica:

- `config.geofences`: lista de geocercas permitidas para disparar la alerta.
- `config.transitions`: lista de transiciones permitidas.

Matching aplicado por el evaluador:

1. `payload.geofence_id` del evento debe estar en `config.geofences`.
2. El tipo de transicion se toma desde `event_type_id`.
3. `config.transitions` acepta dos formas:
   - alias funcionales: `enter`, `exit`.
   - valores UUID directos del `event_type_id`.

### Ejemplo de entrada de geocerca

Evento generado de entrada a geocerca (`event_type_id` corresponde a "Geofence Enter"):

```json
{
  "id": "aaaabbbb-cccc-dddd-eeee-ffff11112222",
  "source_type": "device_message",
  "source_id": "device-001",
  "source_message_id": "550e8400-e29b-41d4-a716-446655440003",
  "unit_id": "99999999-9999-9999-9999-999999999999",
  "event_type_id": "64f9709b-8d4c-4b2e-b1ab-44b015527ba5",
  "payload": {
    "uuid": "550e8400-e29b-41d4-a716-446655440003",
    "device_id": "device-001",
    "msg_class": "POSITION",
    "latitude": -33.8423,
    "longitude": -56.1605,
    "geofence_id": "550e8400-e29b-41d4-a716-446655440001"
  },
  "occurred_at": "2026-04-16T14:40:10Z",
  "source_epoch": null
}
```

Referencia actual de transiciones por UUID:

- `geofence_enter`: `64f9709b-8d4c-4b2e-b1ab-44b015527ba5`
- `geofence_exit`: `23bb5beb-be85-442d-ac5b-c26192b1f86a`
