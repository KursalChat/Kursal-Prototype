use crate::{
    KursalError, Result,
    identity::UserId,
    messaging::enums::{Direction, KursalMessage, MessageId, MessageStatus},
    storage::{Database, TABLE_MESSAGES},
};
use serde::{Deserialize, Serialize};

pub mod enums;

#[derive(Serialize, Deserialize)]
pub struct StoredReaction {
    pub emoji: String,
    pub user_id: UserId,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: MessageId,
    pub contact_id: UserId,
    pub direction: Direction,
    pub payload: KursalMessage,
    pub status: MessageStatus,
    pub timestamp: u64,
    pub raw_ciphertext: Option<Vec<u8>>,
    pub edited: bool,
    pub reactions: Vec<StoredReaction>,
}

impl StoredMessage {
    pub fn save(&self, db: &Database) -> Result<()> {
        let contact_id = hex::encode(self.contact_id.0);
        let message_id = hex::encode(self.id.0);

        let serialized =
            bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))?;

        db.raw_write(
            TABLE_MESSAGES,
            &format!("{contact_id}:{message_id}"),
            &serialized,
        )?;

        Ok(())
    }

    pub fn load(db: &Database, contact_id: &UserId, id: &MessageId) -> Result<Option<Self>> {
        let contact_id = hex::encode(contact_id.0);
        let message_id = hex::encode(id.0);

        let read = db.raw_read(TABLE_MESSAGES, &format!("{contact_id}:{message_id}"))?;

        let result = read
            .map(|e| bincode::deserialize::<StoredMessage>(&e))
            .transpose()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(result)
    }

    pub fn delete(db: &Database, contact_id: &UserId, id: &MessageId) -> Result<()> {
        let contact_id = hex::encode(contact_id.0);
        let message_id = hex::encode(id.0);

        db.raw_delete(TABLE_MESSAGES, &format!("{contact_id}:{message_id}"))?;

        Ok(())
    }

    pub fn load_all(
        db: &Database,
        contact_id: &UserId,
        limit: usize,
        before: Option<&MessageId>,
    ) -> Result<Vec<Self>> {
        let prefix = hex::encode(contact_id.0);
        let start = format!("{}:", prefix);
        let end = before
            .map(|id| format!("{}:{}", prefix, hex::encode(id.0)))
            .unwrap_or_else(|| format!("{};", prefix)); // because ';' is after ':'

        let mut msgs: Vec<Self> = db
            .raw_range(TABLE_MESSAGES, &start, &end, Some(limit))?
            .into_iter()
            .map(|(_, bytes)| {
                bincode::deserialize(&bytes).map_err(|err| KursalError::Storage(err.to_string()))
            })
            .rev()
            .collect::<Result<_>>()?;

        msgs.reverse();

        Ok(msgs)
    }
}
