use dotenv::dotenv;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{signature::Keypair, signer::Signer};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

fn read_keypair() {
    // 读取环境变量
    let key_json = dotenv::var("PRIVATE_KEY").expect("环境变量未设置");

    // 解析 JSON 字符串为 Vec<u8>
    let bytes: Vec<u8> = serde_json::from_str(&key_json).expect("解析失败");

    // 检查长度
    assert_eq!(bytes.len(), 64, "Keypair 需要 64 字节");

    // 转为 Keypair
    let keypair = Keypair::from_bytes(&bytes).expect("生成 Keypair 失败");

    println!("公钥: {}", keypair.pubkey());
}

fn lock() {
    let m = Arc::new(Mutex::new(0));
    let mut handlers = vec![];

    for _ in 1..=10 {
        // let counter = Arc::clone(&m);
        let counter = m.clone();
        let handle = thread::spawn(move || {
            let mut num = counter.lock().unwrap();
            *num += 1;
        });

        handlers.push(handle);
    }

    for h in handlers {
        h.join().unwrap();
    }
    println!("{:?}", *m.lock().unwrap());
}

fn mul_threads() {
    // let mut handlers = vec![];
    // for i in 1..=10 {
    //     let hander = thread::spawn(move || {
    //         println!("{}", i);
    //     });
    //     handlers.push(hander);
    // }

    // for handle in handlers {
    //     handle.join().unwrap();
    // }

    let (tx, rx) = mpsc::channel();

    let tx1 = tx.clone();
    thread::spawn(move || {
        tx1.send(1).unwrap();
        tx1.send(2).unwrap();
        tx1.send(3).unwrap();
    });
    thread::spawn(move || {
        tx.send(7).unwrap();
    });

    for received in rx {
        println!("Got: {received}");
    }

    // let value = rx.recv().unwrap();
    // println!("{}", value);
    thread::sleep(Duration::from_secs_f32(1.0));
}

fn test() -> anyhow::Result<()> {
    dotenv().ok();

    let raw_bytes: [u8; 64] = [
        13, 181, 16, 87, 201, 73, 224, 241, 147, 238, 155, 158, 174, 129, 79, 137, 245, 214, 137,
        112, 201, 159, 208, 129, 212, 184, 170, 241, 78, 201, 14, 34, 20, 101, 8, 213, 50, 227,
        130, 87, 209, 125, 35, 182, 159, 34, 94, 234, 203, 63, 60, 163, 5, 132, 88, 159, 134, 234,
        110, 116, 176, 231, 96, 169,
    ];

    let keypair = Keypair::from_bytes(&raw_bytes)?;

    let pubkey = keypair.pubkey();
    let privkey_base58 = bs58::encode(&raw_bytes).into_string();

    println!("✅ 公钥 (base58): {}", pubkey);
    println!("🔐 私钥 (base58, 64字节): {}", privkey_base58);

    // JSON导出格式（Solana CLI 使用）
    println!(
        "\n📝 JSON (可保存为 id.json)：\n{}",
        serde_json::to_string_pretty(&raw_bytes.to_vec())?
    );

    // let private_key = dotenv::var("PRIVATE_KEY")?;
    // println!("{}", private_key);

    // let wallet = solana_sdk::pubkey::Pubkey::from_str(private_key.as_str())?;
    // let wallet = solana_sdk::signature::Keypair::from_base58_string(&private_key);
    // let keypair = Keypair::from_bytes(private_key.as_bytes()).expect("invalid keypair bytes");
    // let keypair = Keypair::new();
    // println!("{}", keypair.to_base58_string());

    // 假设这是一个 32 字节的私钥
    let keypair_path = "/Users/sxf/.config/solana/id.json";
    let keypair: Keypair =
        solana_sdk::signer::keypair::read_keypair_file(keypair_path).expect("读取 Keypair 失败");
    println!("账户公钥: {}", keypair.pubkey());
    println!("{:?}", keypair.secret());

    let pubkey = keypair.pubkey();

    let client = get_rpc_client();
    let balance = client.get_balance(&pubkey)?;
    println!("{}", balance);

    let recent_blockhash = client.get_latest_blockhash()?;
    println!("recent——blockhash: {}", recent_blockhash);

    let block_number = client.get_block_height()?;
    println!("block height: {}", block_number);

    let block_hash = client.get_latest_blockhash()?;
    println!("{:?}", block_hash);

    // let supply = client.supply()?;
    // println!("{:#?}", supply);

    let slot = client.get_slot()?;
    println!("solt = {}", slot);

    let block_time = client.get_block_time(slot)?;
    println!("block_time = {}", block_time);

    // let limit = 5u64;
    // let start_slot = if slot >= limit { slot - limit } else { slot };
    // let slot_headers = client.get_slot_leaders(start_slot, limit)?;
    // for pubkey_v in slot_headers.iter() {
    //     println!("{}", pubkey_v);
    // }

    // let block_production = client.get_block_production()?;
    // println!("{:#?}", block_production);

    // let cluster_nodes = client.get_cluster_nodes()?;
    // println!("{:#?}", cluster_nodes);
    //
    let epoch = client.get_epoch_info()?;
    println!("{:#?}", epoch);

    let epoch_schedule = client.get_epoch_schedule()?;
    println!("{:?}", epoch_schedule);

    // let leader_schedule = client.get_leader_schedule(Some(0))?;
    // println!("{:?}", leader_schedule);

    Ok(())
}

fn get_rpc_client() -> RpcClient {
    let url = "http://localhost:8899".to_string();
    let commitment_config = CommitmentConfig::confirmed();
    let client = RpcClient::new_with_commitment(url, commitment_config);

    println!("rpc url = {}", client.url());

    client
}
