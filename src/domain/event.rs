use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct IncomingEvent {
    #[serde(alias = "id")]
    pub event_id: Uuid,
    #[serde(default)]
    pub organization_id: Option<Uuid>,
    #[serde(default)]
    pub unit_id: Option<Uuid>,
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub event_type_id: Option<Uuid>,
    #[serde(default)]
    pub source: Option<EventSource>,
    #[serde(default)]
    pub unit: Option<EventUnit>,
    #[serde(default)]
    pub source_epoch: Option<i64>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub received_at: Option<DateTime<Utc>>,
    pub payload: Value,
}

impl IncomingEvent {
    pub fn effective_unit_id(&self) -> Option<Uuid> {
        self.unit_id
            .or_else(|| self.unit.as_ref().map(|unit| unit.id))
    }

    pub fn event_type_or_empty(&self) -> &str {
        self.event_type.as_deref().unwrap_or("")
    }

    pub fn event_label(&self) -> String {
        if let Some(event_type) = &self.event_type {
            return event_type.clone();
        }

        if let Some(event_type_id) = self.event_type_id {
            return event_type_id.to_string();
        }

        "unknown".to_string()
    }

    pub fn received_at_or_occurred_at(&self) -> DateTime<Utc> {
        self.received_at.unwrap_or(self.occurred_at)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct EventSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub id: String,
    #[serde(default)]
    pub message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventUnit {
    pub id: Uuid,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::IncomingEvent;

    #[test]
    fn deserializes_ignition_event_format() {
        let payload = json!({
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
        });

        let event: IncomingEvent = serde_json::from_value(payload).expect("valid event");
        assert_eq!(event.event_type_or_empty(), "ignition_off");
        assert_eq!(
            event.effective_unit_id().expect("unit id"),
            Uuid::parse_str("0c2d17d2-1968-4e58-95c0-c5539ae196fd").expect("uuid")
        );
    }

    #[test]
    fn deserializes_geofence_event_format() {
        let payload = json!({
            "id": "aaaabbbb-cccc-dddd-eeee-ffff11112222",
            "source_type": "device_message",
            "source_id": "device-001",
            "source_message_id": "550e8400-e29b-41d4-a716-446655440003",
            "unit_id": "99999999-9999-9999-9999-999999999999",
            "event_type_id": "ffffffff-ffff-ffff-ffff-000000000003",
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
        });

        let event: IncomingEvent = serde_json::from_value(payload).expect("valid geofence event");
        assert_eq!(
            event.event_type_id.expect("event_type_id").to_string(),
            "ffffffff-ffff-ffff-ffff-000000000003"
        );
        assert_eq!(event.event_type_or_empty(), "");
        assert!(event.received_at.is_none());
        assert_eq!(
            event.effective_unit_id().expect("unit id").to_string(),
            "99999999-9999-9999-9999-999999999999"
        );
    }
}
