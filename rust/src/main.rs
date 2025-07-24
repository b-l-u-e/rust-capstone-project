#![allow(unused)]
use bitcoincore_rpc::bitcoin::{Address, Amount};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;

// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

// You can use calls not provided in RPC lib API using the generic `call` function.
// An example of using the `send` RPC call, which doesn't have exposed API.
// You can also use serde_json `Deserialize` derivation to capture the returned json result.
fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let args = [
        json!([{addr : 100 }]), // recipient address
        json!(null),            // conf target
        json!(null),            // estimate mode
        json!(null),            // fee rate in sats/vb
        json!(null),            // Empty option object
    ];

    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete);
    Ok(send_result.txid)
}

// Helper function to create or load a wallet
fn create_or_load_wallet(rpc: &Client, wallet_name: &str) -> bitcoincore_rpc::Result<()> {
    // First, try to unload the wallet if it's already loaded
    let _ = rpc.unload_wallet(Some(wallet_name));

    // Wait a moment for any locks to clear
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Try to load the wallet first
    match rpc.load_wallet(wallet_name) {
        Ok(_) => {
            println!("Wallet '{}' loaded successfully", wallet_name);
            Ok(())
        }
        Err(_) => {
            // If loading fails, try to create the wallet
            println!("Creating new wallet '{}'", wallet_name);
            match rpc.create_wallet(wallet_name, None, None, None, None) {
                Ok(_) => {
                    println!("Wallet '{}' created successfully", wallet_name);
                    Ok(())
                }
                Err(e) => {
                    // If creation fails, try loading again (might have been created by another process)
                    println!("Wallet creation failed, trying to load again: {:?}", e);
                    rpc.load_wallet(wallet_name)?;
                    println!("Wallet '{}' loaded after retry", wallet_name);
                    Ok(())
                }
            }
        }
    }
}

// Helper function to get wallet balance
fn get_wallet_balance(rpc: &Client, wallet_name: &str) -> bitcoincore_rpc::Result<f64> {
    let balance = rpc.get_balance(None, None)?;
    Ok(balance.to_btc())
}

// Helper function to mine blocks to an address
fn mine_blocks_to_address(
    rpc: &Client,
    address: &Address,
    num_blocks: u64,
) -> bitcoincore_rpc::Result<Vec<String>> {
    let block_hashes = rpc.generate_to_address(num_blocks, address)?;
    Ok(block_hashes
        .into_iter()
        .map(|hash| hash.to_string())
        .collect())
}

// Helper function to convert address to string for display
fn address_to_string(address: &Address) -> String {
    address.to_string()
}

// Helper function to get transaction details
fn get_transaction_details(rpc: &Client, txid: &str) -> bitcoincore_rpc::Result<serde_json::Value> {
    let args = [json!(txid), json!(true)]; // true for verbose output
    rpc.call::<serde_json::Value>("getrawtransaction", &args)
}

// Helper function to get mempool entry
fn get_mempool_entry(rpc: &Client, txid: &str) -> bitcoincore_rpc::Result<serde_json::Value> {
    let args = [json!(txid)];
    rpc.call::<serde_json::Value>("getmempoolentry", &args)
}

// Helper function to get block info
fn get_block_info(rpc: &Client, block_hash: &str) -> bitcoincore_rpc::Result<serde_json::Value> {
    let args = [json!(block_hash)];
    rpc.call::<serde_json::Value>("getblock", &args)
}

fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Get blockchain info
    let blockchain_info = rpc.get_blockchain_info()?;
    println!("Blockchain Info: {:?}", blockchain_info);

    // Create/Load the wallets, named 'Miner' and 'Trader'. Have logic to optionally create/load them if they do not exist or not loaded already.
    println!("\n=== Creating/Loading Wallets ===");
    create_or_load_wallet(&rpc, "Miner")?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    create_or_load_wallet(&rpc, "Trader")?;

    // Generate spendable balances in the Miner wallet. How many blocks needs to be mined?
    println!("\n=== Generating Mining Rewards ===");

    // Create wallet-specific RPC client for Miner wallet
    let miner_rpc = Client::new(
        &format!("{}/wallet/Miner", RPC_URL),
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Generate one address with label "Mining Reward" from the Miner wallet
    let miner_address = miner_rpc
        .get_new_address(Some("Mining Reward"), None)?
        .assume_checked();
    println!(
        "Generated Miner address: {}",
        address_to_string(&miner_address)
    );

    // Mine blocks to get positive balance
    let mut blocks_mined = 0;
    let mut miner_balance = 0.0;

    // Mine blocks until we get a positive balance
    // Note: In regtest, block rewards are 50 BTC per block, but they need 100 confirmations to be spendable
    // So we need to mine at least 101 blocks to have spendable balance
    while miner_balance <= 0.0 {
        blocks_mined += 1;
        println!(
            "Mining block {} to address {}",
            blocks_mined,
            address_to_string(&miner_address)
        );
        let block_hashes = mine_blocks_to_address(&rpc, &miner_address, 1)?;
        println!("Mined block: {}", block_hashes[0]);

        // Check balance after mining
        miner_balance = get_wallet_balance(&miner_rpc, "Miner")?;
        println!(
            "Miner wallet balance after {} blocks: {} BTC",
            blocks_mined, miner_balance
        );
    }

    // Write a short comment describing why wallet balance for block rewards behaves that way.
    println!("\n=== Balance Explanation ===");
    println!(
        "It took {} blocks to get a positive spendable balance because:",
        blocks_mined
    );
    println!("1. Each block reward is 50 BTC in regtest mode");
    println!("2. Block rewards require 100 confirmations to become spendable (mature)");
    println!("3. Therefore, we need to mine at least 101 blocks to have spendable balance from the first block reward");

    // Print the balance of the Miner wallet.
    println!("\n=== Final Miner Balance ===");
    println!("Miner wallet balance: {} BTC", miner_balance);

    // Generate a new address from Trader wallet
    println!("\n=== Setting up Trader Wallet ===");
    let trader_rpc = Client::new(
        &format!("{}/wallet/Trader", RPC_URL),
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;
    let trader_address = trader_rpc
        .get_new_address(Some("Received"), None)?
        .assume_checked();
    println!(
        "Generated Trader address: {}",
        address_to_string(&trader_address)
    );

    // Send 20 BTC from Miner to Trader
    println!("\n=== Sending Transaction ===");
    let send_amount = Amount::from_btc(20.0)?;
    let txid = miner_rpc.send_to_address(
        &trader_address,
        send_amount,
        Some("Payment to Trader"),
        None,
        Some(false),
        Some(false),
        None,
        None,
    )?;
    println!("Transaction sent! TXID: {}", txid);

    // Check transaction in mempool
    println!("\n=== Checking Mempool ===");
    let mempool_entry = get_mempool_entry(&rpc, &txid.to_string())?;
    println!(
        "Mempool entry: {}",
        serde_json::to_string_pretty(&mempool_entry)?
    );

    // Mine 1 block to confirm the transaction
    println!("\n=== Confirming Transaction ===");
    let confirm_block_hashes = mine_blocks_to_address(&rpc, &miner_address, 1)?;
    println!("Confirmation block mined: {}", confirm_block_hashes[0]);

    // Extract all required transaction details
    println!("\n=== Extracting Transaction Details ===");
    let tx_details = get_transaction_details(&rpc, &txid.to_string())?;
    println!(
        "Transaction details: {}",
        serde_json::to_string_pretty(&tx_details)?
    );

    // Get block info for confirmation details
    let block_info = get_block_info(&rpc, &confirm_block_hashes[0])?;

    // Extract required information from transaction details
    let txid_str = tx_details["txid"].as_str().unwrap_or("");

    // Get input details (from the first input)
    let input_txid = tx_details["vin"][0]["txid"].as_str().unwrap_or("");
    let input_vout = tx_details["vin"][0]["vout"].as_u64().unwrap_or(0);

    // Get the previous transaction to find the input address and amount
    let prev_tx_details = get_transaction_details(&rpc, input_txid)?;
    let miner_input_address = prev_tx_details["vout"][input_vout as usize]["scriptPubKey"]
        ["address"]
        .as_str()
        .unwrap_or("");
    let miner_input_amount = prev_tx_details["vout"][input_vout as usize]["value"]
        .as_f64()
        .unwrap_or(0.0);

    // Get output details
    let trader_output_address = tx_details["vout"][0]["scriptPubKey"]["address"]
        .as_str()
        .unwrap_or("");
    let trader_output_amount = tx_details["vout"][0]["value"].as_f64().unwrap_or(0.0);
    let miner_change_address = tx_details["vout"][1]["scriptPubKey"]["address"]
        .as_str()
        .unwrap_or("");
    let miner_change_amount = tx_details["vout"][1]["value"].as_f64().unwrap_or(0.0);

    // Calculate transaction fees (input amount - sum of output amounts)
    let raw_fee = miner_input_amount - trader_output_amount - miner_change_amount;
    let transaction_fees = (raw_fee * 100_000_000.0).round() / 100_000_000.0;

    let block_height = block_info["height"].as_u64().unwrap_or(0);
    let block_hash = block_info["hash"].as_str().unwrap_or("");

    // Write the data to ../out.txt in the specified format given in readme.md
    println!("\n=== Writing Output to File ===");
    let mut output_file = File::create("../out.txt")?;
    writeln!(output_file, "{}", txid_str)?;
    writeln!(output_file, "{}", miner_input_address)?;
    writeln!(output_file, "{}", miner_input_amount)?;
    writeln!(output_file, "{}", trader_output_address)?;
    writeln!(output_file, "{}", trader_output_amount)?;
    writeln!(output_file, "{}", miner_change_address)?;
    writeln!(output_file, "{}", miner_change_amount)?;
    writeln!(output_file, "{}", transaction_fees)?;
    writeln!(output_file, "{}", block_height)?;
    writeln!(output_file, "{}", block_hash)?;

    println!("Output written to ../out.txt successfully!");
    println!("\n=== Summary ===");
    println!("Transaction ID: {}", txid_str);
    println!("Miner's Input Address: {}", miner_input_address);
    println!("Miner's Input Amount: {} BTC", miner_input_amount);
    println!("Trader's Output Address: {}", trader_output_address);
    println!("Trader's Output Amount: {} BTC", trader_output_amount);
    println!("Miner's Change Address: {}", miner_change_address);
    println!("Miner's Change Amount: {} BTC", miner_change_amount);
    println!("Transaction Fees: {} BTC", transaction_fees);
    println!("Block Height: {}", block_height);
    println!("Block Hash: {}", block_hash);

    Ok(())
}
