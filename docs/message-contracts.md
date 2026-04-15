# Contratos de mensajes

Este documento resume que entra y que sale del servicio. No pretende ser una especificacion formal completa, sino una guia practica para entender el intercambio de mensajes.

## 1. Evento de entrada

El consumidor de eventos espera un mensaje que pueda deserializarse como `IncomingEvent` definido en [src/domain/event.rs](../src/domain/event.rs).

Campos principales:

- `event_id`: identificador unico del evento.
- `organization_id`: organizacion asociada, opcional.
- `unit_id`: unidad asociada, opcional.
- `event_type`: tipo de evento de negocio.
- `occurred_at`: cuando ocurrio realmente.
- `received_at`: cuando fue recibido por el sistema previo.
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
- `alert_type`: hoy toma el `event_type` del evento.
- `alert_name`: contiene el `name` de la regla que disparo.
- `payload`: reutiliza el payload del evento.
- `occurred_at`: conserva la fecha del evento original.

Ejemplo:

```json
{
  "id": "4f7db7cf-f8bc-4d9b-bf31-0efff4ac77dd",
  "organization_id": "d7e11a4b-017b-4799-bf66-77f0eab0f91d",
  "unit_id": "0c2d17d2-1968-4e58-95c0-c5539ae196fd",
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
