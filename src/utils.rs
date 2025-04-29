use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use solana_program::system_instruction;
use solana_sdk::{
    hash::Hash, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction,
};
use std::env;
use std::path::PathBuf;

use crate::constants;

/// 计算 anchor 指令 discriminator（前 8 字节的 sha256）
pub fn calculate_discriminator(instruction_name: &str) -> [u8; 8] {
    let full_name = format!("global:{}", instruction_name);
    let hash = Sha256::digest(full_name.as_bytes());

    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

pub fn read_keypair_file(keypair_path: Option<&str>) -> Result<Keypair> {
    let path: PathBuf = match keypair_path {
        Some(p) => PathBuf::from(p),
        None => {
            if let Ok(file) = env::var(constants::KEYPAIR_FILE.as_str()) {
                PathBuf::from(file)
            } else {
                default_solana_keypair_path()
            }
        }
    };

    solana_sdk::signer::keypair::read_keypair_file(&path)
        .map_err(|e| anyhow!("Failed to read keypair file: {}", e))
}

// solana config directory
fn default_solana_keypair_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Cannot find home directory");
    home_dir.join(".config").join("solana").join("id.json")
}

pub fn create_transfer_tx(
    payer: &Keypair,
    from: &Pubkey,
    to: &Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction> {
    let instruction = system_instruction::transfer(from, to, lamports);
    let mut tx = Transaction::new_with_payer(&[instruction], Some(from));
    tx.sign(&[payer], recent_blockhash);
    Ok(tx)
}

pub fn create_tip_tx(
    payer: &Keypair,
    tip_account: &Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction> {
    let instruction = system_instruction::transfer(&payer.pubkey(), tip_account, lamports);
    let mut tx = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    tx.sign(&[payer], recent_blockhash);
    Ok(tx)
}

#[cfg(test)]
mod tests {

    use super::*;
    use tokio; // tokio 的测试宏

    #[tokio::test]
    async fn test_raydium_discriminator_swap_base() {
        let num: u8 = 3;
        println!("{:?}", num.to_le_bytes().to_vec());
        let hex = "09cc2a570300000000848ed55732000000";
        let ret = hex::decode(hex).unwrap();
        println!("ret = {:?}", ret);
        println!("{:?}", ret[..1].to_vec());
        println!("{:?}", ret[1..].to_vec());

        // let discr = "2aec48a2";
        // let expected_hex = "2aec48a2f2182754";
        let discriminator = calculate_discriminator("swap_base_in");
        println!("discriminator  = {:?}", discriminator);
        // let result_hex = hex::encode(discriminator);
        // assert_eq!(result_hex, expected_hex, "Discriminator mismatch");
    }
}
