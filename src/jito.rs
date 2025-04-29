use std::str::FromStr;

use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use jito_sdk_rust::JitoJsonRpcSDK;
use serde_json::json;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
};
use tokio::time::{Duration, sleep};

use crate::constants;

#[derive(Debug)]
struct BundleStatus {
    confirmation_status: Option<String>,
    err: Option<serde_json::Value>,
    transactions: Option<Vec<String>>,
}

pub async fn jito_request(
    recent_blockhash: solana_program::hash::Hash,
    sender: &Keypair,
) -> Result<()> {
    let endpoint = constants::JITO_RPC_ENDPOINT.clone();
    let jito_sdk = JitoJsonRpcSDK::new(&endpoint, None);

    // sender from file
    // let sender: Keypair =
    //     solana_sdk::signer::keypair::read_keypair_file("/Users/sxf/.config/solana/id.json")
    //         .expect("Failed to read keypair file");
    // println!("Sender pubkey: {}", sender.pubkey());

    // ========================= 小费交易
    let serialized_tip_tx = {
        let tip_account_str = jito_sdk.get_random_tip_account().await?;
        let tip_account = Pubkey::from_str(&tip_account_str)?;
        println!("Tips account: {}", tip_account);
        let ix2 = solana_sdk::system_instruction::transfer(&sender.pubkey(), &tip_account, 123456);

        let mut tip_tx =
            solana_sdk::transaction::Transaction::new_with_payer(&[ix2], Some(&sender.pubkey()));
        tip_tx.sign(&[&sender], recent_blockhash);
        base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&tip_tx)?)
    };

    // ============================ 第一笔交易
    let _serialized_tx1 = {
        // ix1
        let receiver = Pubkey::from_str("6Q3D2WvMTo4h34WPWUBphqdZD1K3MVZikJxfrJBaN7gT")?;
        let ix1 = system_instruction::transfer(&sender.pubkey(), &receiver, 2202);

        // ix2 Create memo instruction
        let memo_program_id = Pubkey::from_str("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr")?;
        let memo_ix: Instruction = Instruction::new_with_bytes(
            memo_program_id,
            b"hello, jito bundle! This is a test transaction.",
            vec![AccountMeta::new(sender.pubkey(), true)],
        );

        // tx
        let mut tx = solana_sdk::transaction::Transaction::new_with_payer(
            &[ix1, memo_ix],
            Some(&sender.pubkey()),
        );

        // record blockhash
        tx.sign(&[&sender], recent_blockhash);

        base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&tx)?)
    };

    // ======================= 第二笔交易
    let _serialized_tx2 = {
        let receiver = Pubkey::from_str("89ab91UYbFj8KBJUv1FYgLNzAwaDXdDpE8D4i8vnRy4J")?;
        let ix1 = system_instruction::transfer(&sender.pubkey(), &receiver, 4404);

        let mut tx =
            solana_sdk::transaction::Transaction::new_with_payer(&[ix1], Some(&sender.pubkey()));

        tx.sign(&[&sender], recent_blockhash);

        base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&tx)?)
    };

    // =========================== 打包交易
    let transactions = json!([serialized_tip_tx, _serialized_tx2]);
    let params = json!([transactions, {"encoding": "base64"}]);
    let response = jito_sdk.send_bundle(Some(params), None).await?;

    // Extract bundle UUID from response
    let bundle_uuid = response["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get bundle UUID from response"))?;
    println!("Bundle sent with UUID: {}", bundle_uuid);

    // Confirm bundle status
    let max_retries = 30;
    let retry_delay = Duration::from_secs(2);

    for attempt in 1..=max_retries {
        println!(
            "Checking bundle status (attempt {}/{})",
            attempt, max_retries
        );

        let status_response = jito_sdk
            .get_in_flight_bundle_statuses(vec![bundle_uuid.to_string()])
            .await?;

        if let Some(result) = status_response.get("result") {
            if let Some(value) = result.get("value") {
                if let Some(statuses) = value.as_array() {
                    if let Some(bundle_status) = statuses.get(0) {
                        if let Some(status) = bundle_status.get("status") {
                            match status.as_str() {
                                Some("Landed") => {
                                    println!("Bundle landed on-chain. Checking final status...");
                                    return check_final_bundle_status(&jito_sdk, bundle_uuid).await;
                                }
                                Some("Pending") => {
                                    println!("Bundle is pending. Waiting...");
                                }
                                Some(status) => {
                                    println!("Unexpected bundle status: {}. Waiting...", status);
                                }
                                None => {
                                    println!("Unable to parse bundle status. Waiting...");
                                }
                            }
                        } else {
                            println!("Status field not found in bundle status. Waiting...");
                        }
                    } else {
                        println!("Bundle status not found. Waiting...");
                    }
                } else {
                    println!("Unexpected value format. Waiting...");
                }
            } else {
                println!("Value field not found in result. Waiting...");
            }
        } else if let Some(error) = status_response.get("error") {
            println!("Error checking bundle status: {:?}", error);
        } else {
            println!("Unexpected response format. Waiting...");
        }

        if attempt < max_retries {
            sleep(retry_delay).await;
        }
    }

    Err(anyhow!(
        "Failed to confirm bundle status after {} attempts",
        max_retries
    ))
}

async fn check_final_bundle_status(jito_sdk: &JitoJsonRpcSDK, bundle_uuid: &str) -> Result<()> {
    let max_retries = 30;
    let retry_delay = Duration::from_secs(2);

    for attempt in 1..=max_retries {
        println!(
            "Checking final bundle status (attempt {}/{})",
            attempt, max_retries
        );

        let status_response = jito_sdk
            .get_bundle_statuses(vec![bundle_uuid.to_string()])
            .await?;
        let bundle_status = get_bundle_status(&status_response)?;

        match bundle_status.confirmation_status.as_deref() {
            Some("confirmed") => {
                println!("Bundle confirmed on-chain. Waiting for finalization...");
                check_transaction_error(&bundle_status)?;
            }
            Some("finalized") => {
                println!("Bundle finalized on-chain successfully!");
                check_transaction_error(&bundle_status)?;
                print_transaction_url(&bundle_status);
                return Ok(());
            }
            Some(status) => {
                println!(
                    "Unexpected final bundle status: {}. Continuing to poll...",
                    status
                );
            }
            None => {
                println!("Unable to parse final bundle status. Continuing to poll...");
            }
        }

        if attempt < max_retries {
            sleep(retry_delay).await;
        }
    }

    Err(anyhow!(
        "Failed to get finalized status after {} attempts",
        max_retries
    ))
}

fn get_bundle_status(status_response: &serde_json::Value) -> Result<BundleStatus> {
    status_response
        .get("result")
        .and_then(|result| result.get("value"))
        .and_then(|value| value.as_array())
        .and_then(|statuses| statuses.get(0))
        .ok_or_else(|| anyhow!("Failed to parse bundle status"))
        .map(|bundle_status| BundleStatus {
            confirmation_status: bundle_status
                .get("confirmation_status")
                .and_then(|s| s.as_str())
                .map(String::from),
            err: bundle_status.get("err").cloned(),
            transactions: bundle_status
                .get("transactions")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
        })
}

fn check_transaction_error(bundle_status: &BundleStatus) -> Result<()> {
    if let Some(err) = &bundle_status.err {
        if err["Ok"].is_null() {
            println!("Transaction executed without errors.");
            Ok(())
        } else {
            println!("Transaction encountered an error: {:?}", err);
            Err(anyhow!("Transaction encountered an error"))
        }
    } else {
        Ok(())
    }
}

fn print_transaction_url(bundle_status: &BundleStatus) {
    if let Some(transactions) = &bundle_status.transactions {
        if let Some(tx_id) = transactions.first() {
            println!("Transaction URL: https://solscan.io/tx/{}", tx_id);
        } else {
            println!("Unable to extract transaction ID.");
        }
    } else {
        println!("No transactions found in the bundle status.");
    }
}

#[cfg(test)]
mod tests {
    use crate::jito::jito_request;
    use solana_client::rpc_client::RpcClient;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_jito_bundle() {
        let solana_rpc = RpcClient::new("https://api.testnet.solana.com".to_string());
        let recent_blockhash = solana_rpc.get_latest_blockhash().unwrap();

        // println!("{:?}", recent_blockhash);
        jito_request(recent_blockhash).await.unwrap();
        //
    }
}
