use once_cell::sync::Lazy;
use std::env;

pub static GRPC_ENDPOINT: Lazy<String> = Lazy::new(|| {
    env::var("GRPC_ENDPOINT").unwrap_or_else(|_| {
        "https://solana-testnet-yellowstone-grpc.publicnode.com:443".to_string()
    })
});

pub static JITO_RPC_ENDPOINT: Lazy<String> = Lazy::new(|| {
    env::var("JITO_RPC_ENDPOINT")
        .unwrap_or_else(|_| "https://ny.testnet.block-engine.jito.wtf/api/v1".to_string())
});

pub static KEYPAIR_FILE: Lazy<String> = Lazy::new(|| env::var("KEYPAIR_FILE").unwrap());

pub static PUMP_FUN_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub static RAYDIUM_AAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
