use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Ok, Result, anyhow};
use base64::Engine as _;
use futures_util::StreamExt;
use jito_sdk_rust::JitoJsonRpcSDK;
use log::{debug, info};
use serde_json::json;
use solana_sdk::{hash::Hash, pubkey::Pubkey, signature::Signature, signer::Signer};
use solana_sdk::{
    message::{
        MessageHeader, VersionedMessage,
        v0::{Message as V0Message, MessageAddressTableLookup},
    },
    transaction::VersionedTransaction,
};
use solana_transaction_status::{UiTransactionEncoding, option_serializer::OptionSerializer};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::convert_from;
use yellowstone_grpc_proto::geyser::CommitmentLevel;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{SubscribeRequest, SubscribeRequestFilterTransactions};

use crate::raydium;
use crate::{constants, utils};

pub struct Engine {
    pub jito_sdk: JitoJsonRpcSDK,
}

impl Engine {
    pub async fn new() -> Self {
        Engine {
            jito_sdk: JitoJsonRpcSDK::new(&constants::JITO_RPC_ENDPOINT.clone(), None),
        }
    }

    pub async fn run(&self) -> Result<()> {
        // https://solana-testnet-yellowstone-grpc.publicnode.com:443
        // https://solana-yellowstone-grpc.publicnode.com:443
        let grpc_endpoint = constants::GRPC_ENDPOINT.clone();
        println!("GRPC_ENDPOINT = {}", grpc_endpoint);
        let mut client = GeyserGrpcClient::build_from_shared(grpc_endpoint)?
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .connect()
            .await?;

        let recent_blockhash = Hash::from_str(
            &client
                .get_latest_blockhash(Some(CommitmentLevel::Processed))
                .await?
                .blockhash,
        )
        .unwrap();

        // 支付钱包
        let sender = utils::read_keypair_file(None).unwrap();

        // let (_sink, mut stream) = client.subscribe().await?;
        let account_include = vec![
            // main-beta
            constants::RAYDIUM_AAM_ID.to_string(),
            // raydium devnet
            // "HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8".to_string(),
        ];
        let account_exclude = Vec::new();
        let account_required = Vec::new();
        println!("account_include = {:?}", account_include);

        let mut transactions: HashMap<String, SubscribeRequestFilterTransactions> = HashMap::new();
        transactions.insert(
            "client".to_string(),
            SubscribeRequestFilterTransactions {
                vote: None,
                failed: None,
                signature: None,
                account_include,
                account_exclude,
                account_required,
            },
        );
        let request = SubscribeRequest {
            transactions,
            commitment: Some(CommitmentLevel::Processed.into()),
            ..Default::default()
        };
        let (_sink, mut stream) = client.subscribe_with_request(Some(request)).await?;
        // let version = client.get_version().await?;
        // println!("version = {:#?}", version);

        // 处理接收到的更新
        while let Some(message) = stream.next().await {
            if let Some(update) = message?.update_oneof {
                match update {
                    // tx 类型为 SubscribeUpdateTransaction
                    UpdateOneof::Transaction(tx) => {
                        // tx_info 类型为 SubscribeUpdateTransactionInfo
                        if let Some(tx_info) = tx.transaction {
                            println!(
                                "Signature = {:?}",
                                bs58::encode(&tx_info.signature).into_string()
                            );

                            let allow_sniper = self.allow_sniper(tx_info.clone()).await;
                            if allow_sniper.is_ok() {
                                println!("allow_sniper");
                                let bundle_result =
                                    self.send_bundle(&tx_info, &sender, &recent_blockhash).await;
                                if let Err(err) = bundle_result {
                                    println!("Error sending bundle: {:?}", err);
                                }
                            }
                        }
                    }
                    UpdateOneof::BlockMeta(meta) => {
                        println!("BlockMeta: {:?}", meta);
                    }
                    UpdateOneof::Ping(v) => {
                        println!("Ping received; {:?}", v);
                    }
                    o => {
                        print!("OTHER: {:?}", o);
                    }
                };
            }
        }

        Ok(())
    }

    pub async fn send_bundle(
        &self,
        tx_info: &yellowstone_grpc_proto::geyser::SubscribeUpdateTransactionInfo,
        sender: &solana_sdk::signature::Keypair,
        recent_blockhash: &solana_sdk::hash::Hash,
    ) -> Result<()> {
        // 1. 将监听到的交易转换成一个普通交易，以便于后续打包到 jito
        let _serialized_origin_tx = parsed_tx(tx_info);

        // 2. 创建一个转账交易
        let serialized_transfer_tx = {
            let to = solana_sdk::pubkey::Pubkey::from_str(
                "89ab91UYbFj8KBJUv1FYgLNzAwaDXdDpE8D4i8vnRy4J",
            )?;
            let tx = utils::create_transfer_tx(
                sender,
                &sender.pubkey(),
                &to,
                52345,
                recent_blockhash.clone(),
            )?;
            base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&tx).unwrap())
        };

        // 3. 创建一个jito小费交易
        let serialized_tip_tx = {
            let tip_account = Pubkey::from_str(&self.jito_sdk.get_random_tip_account().await?)?;
            // println!("Tips account: {}", tip_account);

            let tip_tx = utils::create_tip_tx(&sender, &tip_account, 12345, *recent_blockhash)?;

            base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&tip_tx)?)
        };

        // 3. 打包交易
        let transactions = json!([
            serialized_tip_tx,
            serialized_transfer_tx,
            // serialized_origin_tx
        ]); // serialized_origin_tx
        let params = json!([transactions, {"encoding": "base64"}]);
        println!("bundle params = {}", params);
        let response = self.jito_sdk.send_bundle(Some(params), None).await?;
        // // TODO 处理响应
        println!("{:?}", response);
        Ok(())
    }

    pub async fn allow_sniper(
        &self,
        tx_info: yellowstone_grpc_proto::geyser::SubscribeUpdateTransactionInfo,
    ) -> Result<()> {
        let encode_transaction_with_status_meta = convert_from::create_tx_with_meta(tx_info)
            .unwrap()
            .encode(UiTransactionEncoding::Base64, Some(u8::MAX), true)
            .map_err(|e| anyhow!("{}", e));

        if let Some(meta1) = encode_transaction_with_status_meta?.meta {
            if let OptionSerializer::Some(ixs) = meta1.inner_instructions {
                let ixs_len = ixs.len();
                if ixs_len > 0 {
                    debug!("FOUND instructions {:?}", ixs_len);
                }
                let mut idx = 0;
                for inner_ixs in ixs {
                    debug!("inner_ixs: {:?}", inner_ixs);
                    info!("instruaction info == {}", idx);
                    for ix in inner_ixs.instructions {
                        // let ins_result = pumpfun::TargetEvent::try_from(ix) {
                        let ins_result = raydium::AmmInstruction::try_from(ix)?;
                        match ins_result {
                            raydium::AmmInstruction::SwapBaseIn(info) => {
                                // TODO 策略机制，如分析下单详细,考虑滑点，决定是否进行跟单
                                info!("SwapBaseIn: {:?}", info);
                                return Ok(());
                            }
                            raydium::AmmInstruction::SwapBaseOut(info) => {
                                // TODO
                                info!("SwapBaseOut: {:?}", info);
                                return Ok(());
                            }
                            raydium::AmmInstruction::SimulateInfo(simulate_instruction) => {
                                // TODO
                                info!("SimulateInfo: {:?}", simulate_instruction);
                                return Ok(());
                            }
                            raydium::AmmInstruction::Deposit(deposit_instruction) => {
                                // TODO
                                info!("Deposit: {:?}", deposit_instruction);
                                return Ok(());
                            }
                            raydium::AmmInstruction::Withdraw(withdraw_instruction) => {
                                // TODO
                                info!("Withdraw: {:?}", withdraw_instruction);
                                return Ok(());
                            }
                            x => {
                                debug!("OK: {:?}", x);
                            }
                        }
                    }
                    idx += 1;
                }
                if ixs_len > 0 {
                    println!("\n\n");
                }
            }
        }

        Err(anyhow!("Unexpected error"))
    }
}

fn parsed_tx(
    tx_info: &yellowstone_grpc_proto::geyser::SubscribeUpdateTransactionInfo,
) -> Result<String> {
    // 将监听到的grpc响应格式交易转换成一个普通交易，以便于后续打包到 jito
    // 1. 解码 Signature（64 bytes）
    let sig_array: [u8; 64] = tx_info.signature.as_slice().try_into()?; // 注意必须是64字节
    let signature = Signature::from(sig_array);

    // 2. 把 proto::Message 转换成 solana_sdk::Message 《VersionedMessage》
    let origin_message_header = tx_info.transaction.clone().unwrap().message.unwrap();

    // convert_from::create_tx_versioned(tx_info);
    let versioned_message = convert_from::create_message(origin_message_header).unwrap();

    // 3. 组装成 Transaction
    let rebuilt_tx = solana_sdk::transaction::VersionedTransaction {
        signatures: vec![signature],
        message: versioned_message,
    };

    println!("rebuilt_tx = {:?}\n\n\n\n", rebuilt_tx);

    Ok(base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&rebuilt_tx).unwrap()))
}

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;
    use yellowstone_grpc_proto::solana::storage::confirmed_block::MessageAddressTableLookup;

    #[tokio::test]
    async fn test_engine() {
        let input = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
        let bs = bs58::decode(input).into_vec().unwrap();
        println!("Decoded bytes: {:?}", bs);
    }

    #[tokio::test]
    async fn test_bs58() {
        // let account_key = [
        //     39, 245, 50, 118, 56, 33, 10, 103, 136, 229, 202, 96, 137, 27, 228, 49, 156, 69, 149,
        //     203, 173, 178, 188, 223, 214, 48, 126, 12, 2, 98, 70, 160,
        // ];

        // // 将字节数组转换为 Solana 公钥
        // let pubkey = Pubkey::new_from_array(account_key);

        // // 转换为 Base58 地址
        // let base58_address = pubkey.to_string();

        // // 输出 Base58 地址
        // println!("Base58 Address: {}", base58_address);
        //

        // 假设这是 address_table_lookups 中的数据
        // let address_table_lookups = vec![
        //     // 账户查找表
        //     MessageAddressTableLookup {
        //         account_key: vec![
        //             39, 245, 50, 118, 56, 33, 10, 103, 136, 229, 202, 96, 137, 27, 228, 49, 156,
        //             69, 149, 203, 173, 178, 188, 223, 214, 48, 126, 12, 2, 98, 70, 160,
        //         ],
        //         writable_indexes: vec![49, 52, 53, 96, 97, 98, 99],
        //         readonly_indexes: vec![45, 50, 47, 64, 34],
        //     },
        // ];

        // // 假设你要查找的账户索引是 49
        // let target_account_index = 49;

        // // 从 writable_indexes 中查找账户
        // for lookup in address_table_lookups.iter() {
        //     if lookup.writable_indexes.contains(&target_account_index) {
        //         let account_key = lookup.account_key;
        //         let pubkey = Pubkey::from(&account_key.as_slice());
        //         println!("Account found at index 49 with address: {}", pubkey);
        //     }
        // }
    }
}
