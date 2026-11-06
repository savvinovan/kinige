//! Session tokens (ADR-0001 §8.4): 32 random bytes handed out once; the server keeps `crypto_generichash` of them and nothing else.

use crate::error::ApiError;
use crate::wire::{from_b64, to_b64};
use deadpool_postgres::{Pool, Transaction};
use kinige_crypto::ids::auth_hash;
use libsodium_rs::random;

/// A token as the client holds it. Never printed, never logged.
pub struct Token([u8; 32]);

impl Token {
    pub fn random() -> Token {
        let mut bytes = [0u8; 32];
        random::fill_bytes(&mut bytes);
        Token(bytes)
    }

    /// From `Authorization: Bearer <token>`'s text.
    pub fn parse(text: &str) -> Option<Token> {
        <[u8; 32]>::try_from(from_b64(text)?.as_slice())
            .ok()
            .map(Token)
    }

    pub fn encode(&self) -> String {
        to_b64(&self.0)
    }

    pub fn hash(&self) -> Result<[u8; 32], ApiError> {
        auth_hash(&self.0).map_err(|_| ApiError::Internal)
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        libsodium_rs::utils::memzero(&mut self.0);
    }
}

/// Opens a session for `account` inside `tx` and returns its token.
pub async fn open(tx: &Transaction<'_>, account: &str, now_ms: u64) -> Result<Token, ApiError> {
    let token = Token::random();
    let now = super::to_i64(now_ms, "clock")?;
    tx.execute(
        "INSERT INTO sessions (token_hash, account_id, created_ms) VALUES ($1, $2, $3)",
        &[&token.hash()?.as_slice(), &account, &now],
    )
    .await?;
    Ok(token)
}

/// The account a token belongs to; `Auth` for a token the server does not know.
pub async fn account_of(pool: &Pool, token: &Token) -> Result<String, ApiError> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT account_id FROM sessions WHERE token_hash = $1",
            &[&token.hash()?.as_slice()],
        )
        .await?;
    match row {
        Some(row) => Ok(row.try_get("account_id")?),
        None => Err(ApiError::Auth),
    }
}

/// Ends one session (logout). `Auth` if the token was not a session.
pub async fn close(pool: &Pool, token: &Token) -> Result<(), ApiError> {
    let client = pool.get().await?;
    let removed = client
        .execute(
            "DELETE FROM sessions WHERE token_hash = $1",
            &[&token.hash()?.as_slice()],
        )
        .await?;
    if removed == 0 {
        return Err(ApiError::Auth);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_round_trips_and_garbage_does_not_parse() {
        let token = Token::random();
        let text = token.encode();
        let again = Token::parse(&text).expect("parse");
        assert_eq!(again.hash().ok(), token.hash().ok());
        for bad in ["", "abc", "!!!", &format!("{text}x")] {
            assert!(Token::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn two_tokens_differ_and_so_do_their_hashes() {
        let (a, b) = (Token::random(), Token::random());
        assert_ne!(a.encode(), b.encode());
        assert_ne!(a.hash().ok(), b.hash().ok());
    }
}
