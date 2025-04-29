use anyhow::anyhow;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::{bs58, pubkey::Pubkey};
use solana_transaction_status::{UiCompiledInstruction, UiInstruction};

const PUMPFUN_CREATE_EVENT: [u8; 8] = [27, 114, 169, 77, 222, 235, 99, 118];
const PUMPFUN_COMPLETE_EVENT: [u8; 8] = [95, 114, 97, 156, 212, 46, 152, 8];
const PUMPFUN_TRADE_EVENT: [u8; 8] = [189, 219, 127, 211, 78, 230, 97, 238];

// IDL: https://github.com/cfanbo/pumpdotfun-sdk/blob/main/src/IDL/pump-fun.json
// 这里监听的是事件
#[derive(Debug, Clone)]
pub enum TargetEvent {
    PumpfunBuy(TradeEvent),
    PumpfunSell(TradeEvent),
    PumpfunCreate(CreateEvent),
    PumpfunComplete(CompleteEvent),
}

impl TryFrom<UiInstruction> for TargetEvent {
    type Error = anyhow::Error;

    fn try_from(inner_instruction: UiInstruction) -> Result<Self, Self::Error> {
        // 处理每一条指令
        match inner_instruction {
            solana_transaction_status::UiInstruction::Compiled(ui_compiled_instruction) => {
                if let Some(create) =
                    CreateEvent::try_from_compiled_instruction(&ui_compiled_instruction)
                {
                    return Ok(TargetEvent::PumpfunCreate(create));
                }
                if let Some(complete) =
                    CompleteEvent::try_from_compiled_instruction(&ui_compiled_instruction)
                {
                    return Ok(Self::PumpfunComplete(complete));
                }
                if let Some(trade) =
                    TradeEvent::try_from_compiled_instruction(&ui_compiled_instruction)
                {
                    if trade.is_buy {
                        return Ok(TargetEvent::PumpfunBuy(trade));
                    } else {
                        return Ok(TargetEvent::PumpfunSell(trade));
                    }
                }
            }
            _ => {}
        }
        return Err(anyhow!("failed to convert to target tx"));
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CreateEvent {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint: Pubkey,
    pub bonding_curve: Pubkey,
    pub user: Pubkey,
}

impl CreateEvent {
    pub fn try_from_compiled_instruction(
        ui_compiled_instruction: &UiCompiledInstruction,
    ) -> Option<CreateEvent> {
        let data = bs58::decode(ui_compiled_instruction.data.clone())
            .into_vec()
            .unwrap();
        if data.len() > 16 && data[8..16].eq(&PUMPFUN_CREATE_EVENT) {
            match CreateEvent::try_from_slice(&data[16..]) {
                Ok(event) => return Some(event),
                Err(_) => return None,
            }
        } else {
            return None;
        }
    }
}

#[derive(Debug, BorshSerialize, Clone, BorshDeserialize, Copy)]
pub struct CompleteEvent {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub bonding_curve: Pubkey,
    pub timestamp: i64,
}

impl CompleteEvent {
    pub fn try_from_compiled_instruction(
        ui_compiled_instruction: &UiCompiledInstruction,
    ) -> Option<CompleteEvent> {
        let data = bs58::decode(ui_compiled_instruction.data.clone())
            .into_vec()
            .unwrap();
        if data.len() > 16 && data[8..16].eq(&PUMPFUN_COMPLETE_EVENT) {
            match CompleteEvent::try_from_slice(&data[16..]) {
                Ok(event) => return Some(event),
                Err(_) => return None,
            }
        } else {
            return None;
        }
    }
}

#[derive(Debug, BorshSerialize, Clone, BorshDeserialize)]
pub struct BuyArgs {
    pub amount: u64,
    pub max_sol_cost: u64,
}

#[derive(Debug, BorshSerialize, Clone, BorshDeserialize, Copy)]
pub struct TradeEvent {
    pub mint: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: bool,
    pub user: Pubkey,
    pub timestamp: i64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
}

impl TradeEvent {
    pub fn try_from_compiled_instruction(
        ui_compiled_instruction: &UiCompiledInstruction,
    ) -> Option<TradeEvent> {
        let data = bs58::decode(ui_compiled_instruction.data.clone())
            .into_vec()
            .unwrap();
        if data.len() > 16 && data[8..16].eq(&PUMPFUN_TRADE_EVENT) {
            match TradeEvent::try_from_slice(&data[16..]) {
                Ok(event) => return Some(event),
                Err(_) => return None,
            }
        } else {
            return None;
        }
    }
}

#[derive(Debug, Clone)]
pub enum Reason {
    USUAL,
    WHITE,
}

#[derive(Debug, Clone)]
pub struct BuyAndReason {
    pub mint: Pubkey,
    pub reason: Reason,
    pub price: f32,
}

#[tokio::test]
async fn test() {
    let data = "2K7nL28PxCW8ejnyCeuMpbYAmP2pnuyvkxEQgp79nsKJzbKfMq82LAVFjwFY1xYhKmuaA8H3M5xLfFnF85Xbai9s9aaCyDETZgWMQJayFp8t1HM9ihUxb1TCcsXYVsNKDqaGANFoxSEAPLvpAXJVQHTNyAMxFcgM9s3knpLcDTYtGe7Ufq3WZ9kvAGdd";
    let data = bs58::decode(data.as_bytes()).into_vec().unwrap();
    println!("data {:?}", data);
    let result = TradeEvent::try_from_slice(&data[16..]).unwrap();
    println!("result {:?}", result);
}
