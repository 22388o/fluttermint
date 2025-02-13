use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use bitcoin::hashes::sha256;
use lazy_static::lazy_static;
use tokio::runtime;
use tokio::sync::Mutex;

use crate::client::Client;

lazy_static! {
    static ref RUNTIME: runtime::Runtime = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");
}

mod global_client {
    use super::*;

    static GLOBAL_CLIENT: Mutex<Option<Arc<Client>>> = Mutex::const_new(None);

    pub async fn get() -> Result<Arc<Client>> {
        let client = GLOBAL_CLIENT
            .lock()
            .await
            .as_ref()
            .ok_or(anyhow!("join a federation first"))?
            .clone();
        Ok(client)
    }

    pub async fn is_some() -> bool {
        GLOBAL_CLIENT.lock().await.is_some()
    }

    pub async fn remove() -> Result<()> {
        *GLOBAL_CLIENT.lock().await = None;
        tracing::info!("Client removed");
        Ok(())
    }

    pub async fn set(client: Arc<Client>) {
        *GLOBAL_CLIENT.lock().await = Some(client);
    }
}

/// If this returns true, user has joined a federation. Otherwise they haven't.
pub fn init(path: String) -> Result<bool> {
    tracing::info!("called init()");
    // Configure logging
    #[cfg(target_os = "android")]
    use tracing_subscriber::{layer::SubscriberExt, prelude::*, Layer};
    #[cfg(target_os = "android")]
    tracing_subscriber::registry()
        .with(
            paranoid_android::layer("com.justinmoon.fluttermint")
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .try_init()
        .unwrap_or_else(|error| tracing::info!("Error installing logger: {}", error));

    #[cfg(target_os = "ios")]
    use tracing_subscriber::{layer::SubscriberExt, prelude::*, Layer};
    #[cfg(target_os = "ios")]
    tracing_subscriber::registry()
        .with(
            tracing_oslog::OsLogger::new(
                "com.justinmoon.fluttermint",
                "INFO", // I don't know what this does ...
            )
            .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .try_init()
        .unwrap_or_else(|error| tracing::info!("Error installing logger: {}", error));

    #[cfg(target_os = "macos")]
    tracing_subscriber::fmt()
        .try_init()
        .unwrap_or_else(|error| tracing::info!("Error installing logger: {}", error));

    RUNTIME.block_on(async {
        if global_client::is_some().await {
            return Ok(true);
        };
        global_client::remove().await?;
        let filename = Path::new(&path).join("client.db");
        let db = sled::open(&filename)?.open_tree("mint-client")?;
        if let Some(client) = Client::try_load(Box::new(db)).await? {
            let client = Arc::new(client);
            global_client::set(client.clone()).await;
            // TODO: kill the poll task on leave
            tokio::spawn(async move { client.poll().await });
            return Ok(true);
        }
        Ok(false)
    })
}

pub fn join_federation(user_dir: String, config_url: String) -> Result<()> {
    RUNTIME.block_on(async {
        global_client::remove().await?;
        let filename = Path::new(&user_dir).join("client.db");
        std::fs::remove_dir_all(&filename)?;
        let db = sled::open(&filename)?.open_tree("mint-client")?;
        let client = Arc::new(Client::new(Box::new(db), &config_url).await?);
        global_client::set(client.clone()).await;
        // TODO: kill the poll task on leave
        tokio::spawn(async move { client.poll().await });
        Ok(())
    })
}

pub fn leave_federation() -> Result<()> {
    // delete the database (their ecash tokens will disappear ... this shouldn't be done lightly ...)
    // set CLIENT to None
    Ok(())
}

pub fn balance() -> Result<u64> {
    RUNTIME.block_on(async { Ok(global_client::get().await?.balance().await) })
}

pub fn pay(bolt11: String) -> Result<()> {
    RUNTIME.block_on(async { global_client::get().await?.pay(bolt11).await })
}

pub fn decode_invoice(bolt11: String) -> Result<String> {
    crate::client::decode_invoice(bolt11)
}

pub fn invoice(amount: u64, description: String) -> Result<String> {
    RUNTIME.block_on(async {
        global_client::get()
            .await?
            .invoice(amount, description)
            .await
    })
}

#[derive(Clone, Debug)]
pub struct MyPayment {
    pub invoice: String, // FIXME: we should pass the real invoice here
    pub paid: bool,
}

pub fn fetch_payment(payment_hash: String) -> Result<MyPayment> {
    let hash: sha256::Hash = payment_hash.parse()?;
    RUNTIME.block_on(async {
        let payment = global_client::get()
            .await?
            .client
            .ln_client()
            .fetch_payment(&hash)
            .ok_or(anyhow!("payment not found"))?;
        Ok(MyPayment {
            invoice: decode_invoice(payment.invoice.to_string())?,
            paid: payment.paid,
        })
    })
}

pub fn list_payments() -> Result<Vec<MyPayment>> {
    RUNTIME.block_on(async {
        let payments = global_client::get()
            .await?
            .client
            .ln_client()
            .list_payments()
            .iter()
            .map(|payment| MyPayment {
                // FIXME: don't expect
                invoice: decode_invoice(payment.invoice.to_string())
                    .expect("couldn't decode invoice"),
                paid: payment.paid,
            })
            .collect();
        Ok(payments)
    })
}
