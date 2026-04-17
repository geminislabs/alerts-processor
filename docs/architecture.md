# Arquitectura del Alert Processor

## Objetivo del servicio

Este servicio convierte eventos operativos en alertas de negocio.

En lugar de pedirle a otros sistemas que entiendan todas las reglas y combinaciones posibles, `alert-processor` centraliza esa decision. Su trabajo es responder una pregunta sencilla:

> dado un evento entrante, existe alguna regla activa para esta unidad que deba generar una alerta?

## Vista general

```text
                 +---------------------------+
                 |        PostgreSQL         |
                 | reglas activas iniciales  |
                 +-------------+-------------+
                               |
                               v
                 +---------------------------+
                 |       RulesCache          |
                 | unit_id -> [reglas]       |
                 +------+--------------------+
                        ^
                        |
    topic de updates ---+

    topic de eventos ------------------------------+
                                                   v
                                       +----------------------+
                                       |      Processor       |
                                       | coordina ambos loops |
                                       +----------+-----------+
                                                  |
                                                  v
                                       +----------------------+
                                       | EvaluatorRegistry    |
                                       | rule_type -> logica  |
                                       +----------+-----------+
                                                  |
                                                  v
                                       +----------------------+
                                       |  Kafka alerts topic  |
                                       +----------------------+
```

## Recorrido paso a paso

### 1. Arranque

En [src/main.rs](../src/main.rs) la aplicacion hace la inicializacion del proceso:

1. configura logging con `tracing`,
2. lee configuracion desde variables de entorno,
3. crea un pool de PostgreSQL,
4. carga reglas activas desde base de datos,
5. construye la cache en memoria,
6. registra los evaluadores conocidos,
7. crea consumidores y productor Kafka,
8. entrega todo al `Processor`.

## 2. Flujo de eventos

El loop principal de eventos vive en [src/app/processor.rs](../src/app/processor.rs).

Por cada mensaje recibido desde el topic de eventos:

1. intenta deserializar el payload,
2. identifica la unidad del evento,
3. consulta la cache para obtener reglas asociadas a esa unidad,
4. para cada regla busca un evaluador por `rule_type`,
5. si el evaluador dispara, publica la alerta en Kafka.

Si no hay reglas para la unidad, el evento se descarta sin error funcional.

## 3. Flujo de actualizacion de reglas

El mismo `Processor` corre un segundo loop para mensajes del topic de actualizaciones de reglas.

Ese flujo evita reiniciar el servicio cada vez que cambia una regla.

Tipos de mensaje soportados hoy:

- `UPSERT`: crea o reemplaza una regla en cache.
- `DELETE`: elimina una regla de la cache.

La cache compara `updated_at` para no sobrescribir una version nueva con una version vieja.

## 4. Cache en memoria

La cache esta implementada en [src/cache/rules_cache.rs](../src/cache/rules_cache.rs).

Usa dos estructuras internas:

- `by_unit`: mapa rapido de `unit_id` a lista de reglas aplicables.
- `by_rule`: seguimiento de la version actual de cada regla para manejar updates y deletes.

Esta doble vista permite dos cosas a la vez:

- buscar reglas por unidad rapidamente,
- saber que version de una regla es la vigente.

## 5. Base de datos

El acceso inicial a reglas se hace mediante [src/db/repository.rs](../src/db/repository.rs).

La consulta:

- toma reglas desde `public.alert_rules`,
- une organizaciones y unidades relacionadas,
- filtra organizaciones activas,
- filtra reglas activas,
- agrega las unidades por regla.

Importante: la base de datos se usa para la carga inicial. La sincronizacion posterior depende de Kafka.

## 6. Evaluadores

La interfaz comun esta en [src/engine/evaluator.rs](../src/engine/evaluator.rs).

Cada evaluador debe implementar:

```rust
fn evaluate(&self, event: &IncomingEvent, rule: &Rule) -> Option<Alert>
```

Interpretacion sencilla:

- devuelve `Some(Alert)` cuando la regla dispara,
- devuelve `None` cuando el evento no cumple la condicion.

El registro de evaluadores esta en [src/engine/registry.rs](../src/engine/registry.rs).

Eso evita `if` o `match` gigantes distribuidos por el proyecto. Cada tipo de regla puede crecer como modulo independiente.

### Evaluador `ignition_off`

Definido en [src/engine/ignition.rs](../src/engine/ignition.rs).

Comportamiento:

- lee `rule.config["event"]`,
- compara contra `event.event_type`,
- si coincide, genera una alerta con el payload original del evento.

### Evaluador `geofence`

Definido en [src/engine/geofence.rs](../src/engine/geofence.rs).

Comportamiento:

- lee `rule.config.geofences` como lista de UUIDs permitidos,
- lee `rule.config.transitions` (por alias `enter`/`exit` o UUID de `event_type_id`),
- toma `payload.geofence_id` y `event_type_id` del evento,
- genera alerta cuando ambos matches son verdaderos.

## 7. Kafka

Los consumidores y productor estan en [src/kafka/consumer.rs](../src/kafka/consumer.rs) y [src/kafka/producer.rs](../src/kafka/producer.rs).

Aspectos importantes:

- autenticacion SASL configurable,
- `auto.offset.reset` configurable,
- `enable.auto.commit = true`,
- soporte para payload JSON directo,
- soporte para payload doblemente serializado como string JSON.

## 8. Observabilidad y comportamiento operativo

El servicio registra logs estructurados con `tracing`.

Hay logs para:

- arranque y configuracion general,
- conexion a PostgreSQL,
- cantidad de reglas cargadas,
- recepcion de mensajes Kafka,
- alertas publicadas,
- errores de parseo o publicacion,
- updates de reglas aplicados o descartados por stale.

El proceso no se detiene por un solo mensaje invalido. La estrategia actual es registrar el error y seguir.

## 9. Decisiones de diseño faciles de pasar por alto

### Las reglas se filtran por unidad

No se recorren todas las reglas para cada evento. Primero se localiza la unidad y solo se evalua el subconjunto relevante.

### La cache se mantiene viva por Kafka

Esto reduce carga sobre la base de datos y evita reinicios al modificar reglas.

### El motor es extensible por `rule_type`

Agregar una nueva regla normalmente implica:

1. crear un nuevo evaluador,
2. registrarlo en el `EvaluatorRegistry`,
3. acordar el formato de `rule.config`.

## 10. Riesgos y limites actuales

- si el topic de actualizaciones falla o deja de recibir eventos, la cache puede quedar desactualizada respecto a la base,
- no existe deduplicacion explicita de alertas en el servicio,
- el commit de offsets es automatico, por lo que la semantica exacta de reproceso depende de Kafka y de la posicion del fallo.
