use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{body::Bytes, http::StatusCode, response::IntoResponse};
use rand::{distributions, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::LoginPacket;

const MIN_PASSWORD_LENGTH: usize = 6;
const TOKEN_LENGTH: usize = 20;
const DATABASE_PATH: &str = "database";
const ACCOUNTS_TREE: &str = "accounts";

nestify::nest! {
    #[derive(Debug, Serialize, Deserialize, Clone)]*
    struct AuthDatabase {
        accounts: Vec<struct Account {
            admin: bool,
            username: String,
            password_hash: String,
            tokens: Vec<struct Token {
                token: String,
                last_used: OffsetDateTime,
            }>,
        }>,
    }
}

pub async fn login_server(packet: Bytes) -> impl IntoResponse {
    match bincode::deserialize::<LoginPacket>(&packet) {
        Ok(packet) => match login_impl(&packet) {
            Ok(token) => (StatusCode::OK, token),
            Err(e) => {
                log::error!("Failed to login: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        },
        Err(e) => {
            log::error!("Failed to deserialise login packet: {:?}", e);
            (StatusCode::BAD_REQUEST, String::new())
        }
    }
}

/// Login to account, returning a token
/// If no password is set, it will set the password
/// If no accounts exist, it will create an admin account
fn login_impl(packet: &LoginPacket) -> Result<String> {
    // Initialize the sled database
    let db = sled::open(DATABASE_PATH).map_err(|e| anyhow!("Failed to open database: {:?}", e))?;
    let tree = db
        .open_tree(ACCOUNTS_TREE)
        .map_err(|e| anyhow!("Failed to open tree: {:?}", e))?;

    // Create initial admin account if no accounts exist
    if tree.is_empty() {
        if packet.password.len() < MIN_PASSWORD_LENGTH {
            return Err(anyhow!(
                "Password must be at least {} characters long",
                MIN_PASSWORD_LENGTH
            ));
        }

        // Hash the password
        let password_hash = Argon2::default()
            .hash_password(
                packet.password.as_bytes(),
                &SaltString::generate(&mut OsRng),
            )
            .map_err(|_| anyhow!("Failed to hash password"))?
            .to_string();

        // Create a new admin account
        let (token_entry, token) = generate_token();
        let new_account = Account {
            admin: true,
            username: packet.username.clone(),
            password_hash,
            tokens: vec![token_entry],
        };

        // Serialize and save the admin account to the database
        let account_bytes = bincode::serialize(&new_account)
            .map_err(|e| anyhow!("Failed to serialize account: {:?}", e))?;
        tree.insert(packet.username.as_bytes(), account_bytes)
            .map_err(|e| anyhow!("Failed to save admin account: {:?}", e))?;

        return Ok(format!("Admin Account Created|{token}"));
    }

    // Retrieve account data using username as the key
    let account_data = tree
        .get(packet.username.as_bytes())
        .map_err(|e| anyhow!("Database access error: {:?}", e))?;

    if let Some(account_bytes) = account_data {
        // Deserialize the account data
        let mut account: Account = bincode::deserialize(&account_bytes)
            .map_err(|_| anyhow!("Incorrect username or password"))?;

        if account.password_hash.is_empty() {
            // This is a new account setup case
            if packet.password.len() < MIN_PASSWORD_LENGTH {
                return Err(anyhow!("Password must be at least 6 characters long"));
            }

            // Hash the new password
            let password_hash = Argon2::default()
                .hash_password(
                    packet.password.as_bytes(),
                    &SaltString::generate(&mut OsRng),
                )
                .map_err(|_| anyhow!("Failed to hash password"))?
                .to_string();

            // Update the account with the new password and add a token
            let (token_entry, token) = generate_token();
            account.tokens.push(token_entry);
            account.password_hash = password_hash;

            // Serialize and save the updated account back to the database
            let updated_account_bytes = bincode::serialize(&account)
                .map_err(|e| anyhow!("Failed to serialize account: {:?}", e))?;
            tree.insert(packet.username.as_bytes(), updated_account_bytes)
                .map_err(|e| anyhow!("Failed to update account: {:?}", e))?;

            return Ok(format!("Admin Set|{token}"));
        }

        // Verify password for an existing account
        let parsed_hash = PasswordHash::new(&account.password_hash)
            .map_err(|_| anyhow!("Incorrect username or password"))?;

        if Argon2::default()
            .verify_password(packet.password.as_bytes(), &parsed_hash)
            .is_ok()
        {
            // Create and store a new token
            let (token_entry, token) = generate_token();
            account.tokens.push(token_entry);

            // Serialize and save the updated account back to the database
            let updated_account_bytes = bincode::serialize(&account)
                .map_err(|e| anyhow!("Failed to serialize account: {:?}", e))?;
            tree.insert(packet.username.as_bytes(), updated_account_bytes)
                .map_err(|e| anyhow!("Failed to update account: {:?}", e))?;

            return Ok(token);
        }
    }
    Err(anyhow!("Incorrect username or password"))
}

/// Helper function to generate a random token
fn generate_token() -> (Token, String) {
    let new_token: String = thread_rng()
        .sample_iter(&distributions::Alphanumeric)
        .take(TOKEN_LENGTH)
        .map(char::from)
        .collect();
    let token = Token {
        token: new_token.clone(),
        last_used: OffsetDateTime::now_utc(),
    };
    (token, new_token)
}

/// Verify tokens, updating the `last_used`
pub fn verify_token(input_token: &str) -> Result<bool> {
    // Open the database
    let db = sled::open(DATABASE_PATH).map_err(|e| anyhow!("Failed to open database: {:?}", e))?;
    let tree = db
        .open_tree(ACCOUNTS_TREE)
        .map_err(|e| anyhow!("Failed to open tree: {:?}", e))?;

    // Iterate through all accounts
    for item in &tree {
        let (key, account_bytes) =
            item.map_err(|e| anyhow!("Failed to iterate over accounts: {:?}", e))?;

        // Deserialize account data
        let mut account: Account = bincode::deserialize(&account_bytes)
            .map_err(|_| anyhow!("Failed to deserialize account data"))?;

        // Check for token match and update last_used
        if let Some(token_entry) = account
            .tokens
            .iter_mut()
            .find(|token| token.token == input_token)
        {
            // Update last_used field to the current time
            token_entry.last_used = OffsetDateTime::now_utc();

            // Serialize and save the updated account back to the database
            let updated_account_bytes = bincode::serialize(&account)
                .map_err(|e| anyhow!("Failed to serialize account: {:?}", e))?;
            tree.insert(key, updated_account_bytes)
                .map_err(|e| anyhow!("Failed to update account: {:?}", e))?;

            return Ok(true);
        }
    }

    Ok(false)
}
