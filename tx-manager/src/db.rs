use ethers::types::TransactionReceipt;
use redis::{Client, Commands, Connection, RedisResult};

use crate::transaction::Transaction;

enum DatabaseValue {
    Receipt(TransactionReceipt),
}

#[derive(Debug)]
pub enum DatabaseError {
    TODO,
}

pub struct Database {
    con: Connection,
}

impl Database {
    pub fn new() -> Result<Self, DatabaseError> {
        return Ok(Self {
            con: Client::open("redis://127.0.0.1/6379")
                .map_err(|_| DatabaseError::TODO)?
                .get_connection()
                .map_err(|_| DatabaseError::TODO)?,
        });
    }

    pub fn get_transaction_receipt_for(
        &self,
        transaction: &Transaction, // TODO
    ) -> Result<TransactionReceipt, DatabaseError> {
        return from_string(
            (self.con.get("todo") as RedisResult<String>)
                .map_err(|_| DatabaseError::TODO)?,
        );
    }
}

fn from_string(s: String) -> Result<TransactionReceipt, DatabaseError> {
    return serde_json::from_str(&s).map_err(|_| DatabaseError::TODO);
}

/*
if let Value::Status(s) = v {
    let receipt: TransactionReceipt =
        serde_json::from_str(s).map_err(|_| DatabaseError::TODO)?;
}
return Err(RedisError::from((ErrorKind::TypeError, "Expected string")));
*/
