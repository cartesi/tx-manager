use std::ops::Deref;

use ethers::types::TransactionReceipt;

use redis::{
    Client, Commands, Connection, ErrorKind, FromRedisValue, RedisError,
    RedisResult, Value,
};

use crate::transaction::Transaction;

#[derive(Debug)]
pub enum ReceiptDatabaseError {
    TODO,
}

pub struct ReceiptDatabase {
    con: Connection,
}

impl ReceiptDatabase {
    pub fn new() -> Result<Self, ReceiptDatabaseError> {
        return Ok(Self {
            con: Client::open("redis://127.0.0.1/6379")
                .map_err(|_| ReceiptDatabaseError::TODO)?
                .get_connection()
                .map_err(|_| ReceiptDatabaseError::TODO)?,
        });
    }

    pub fn get_receipt(
        &mut self,
        transaction: &Transaction,
    ) -> Result<TransactionReceipt, ReceiptDatabaseError> {
        let receipt: Receipt = self
            .con
            .get(transaction.label)
            .map_err(|_| ReceiptDatabaseError::TODO)?;
        return Ok(receipt.0);
    }
}

struct Receipt(TransactionReceipt);

impl FromRedisValue for Receipt {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        if let Value::Status(s) = v {
            if let Ok(receipt) = serde_json::from_str(s).map(Receipt) {
                return Ok(receipt);
            }
        }
        return Err(RedisError::from((
            ErrorKind::TypeError,
            "Could not convert the TransactionReceipt",
        )));
    }
}
