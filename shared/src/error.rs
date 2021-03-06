use ckb_db::kvdb::Error as DBError;
use failure::Fail;

#[derive(Debug, PartialEq, Clone, Eq, Fail)]
pub enum SharedError {
    #[fail(display = "InvalidInput")]
    InvalidInput,
    #[fail(display = "InvalidOutput")]
    InvalidOutput,
    #[fail(display = "InvalidTransaction: {}", _0)]
    InvalidTransaction(String),
    #[fail(display = "InvalidParentBlock")]
    InvalidParentBlock,
    #[fail(display = "DB error: {}", _0)]
    DB(DBError),
}
