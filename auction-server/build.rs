use {
    anchor_lang_idl::{
        convert::convert_idl,
        types::{
            Idl,
            IdlInstructionAccountItem,
        },
    },
    std::{
        fs,
        process::{
            Command,
            Stdio,
        },
    },
};

fn build_evm_contracts() {
    let contract_setup = r#"
        cd ../contracts/evm
        forge build --via-ir
    "#;
    println!("cargo:rerun-if-changed=../contracts/evm");
    // Build the contracts and generate the ABIs. This is required for abigen! macro expansions to work.
    let output = Command::new("sh")
        .args(["-c", contract_setup])
        .output()
        .expect("Failed to run build contracts command");
    if !output.status.success() {
        panic!(
            "Failed to build contracts: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        println!(
            "Built all solidity contracts  {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

const SUBMIT_BID_INSTRUCTION_SVM: &str = "submit_bid";
const SUBMIT_BID_PERMISSION_ACCOUNT_SVM: &str = "permission";
const SUBMIT_BID_ROUTER_ACCOUNT_SVM: &str = "router";

const SWAP_INSTRUCTION_SVM: &str = "swap";
const SWAP_ROUTER_TOKEN_ACCOUNT_SVM: &str = "router_fee_receiver_ta";
const SWAP_SEARCHER_ACCOUNT_SVM: &str = "searcher";
const SWAP_USER_WALLET_ACCOUNT_SVM: &str = "user";
const SWAP_MINT_SEARCHER_ACCOUNT_SVM: &str = "mint_searcher";
const SWAP_MINT_USER_ACCOUNT_SVM: &str = "mint_user";
const SWAP_TOKEN_PROGRAM_SEARCHER_SVM: &str = "token_program_searcher";
const SWAP_TOKEN_PROGRAM_USER_SVM: &str = "token_program_user";
const IDL_LOCATION: &str = "../contracts/svm/target/idl/express_relay.json";

fn extract_account_position(idl: Idl, instruction_name: &str, account_name: &str) -> usize {
    let instruction = idl
        .instructions
        .iter()
        .find(|i| i.name == instruction_name)
        .unwrap_or_else(|| panic!("Instruction {} not found in IDL", instruction_name));
    instruction
        .accounts
        .iter()
        .position(|a| match a {
            IdlInstructionAccountItem::Single(a) => a.name == account_name,
            IdlInstructionAccountItem::Composite(a) => a.name == account_name,
        })
        .unwrap_or_else(|| {
            panic!(
                "Account {} not found in instruction {}",
                account_name, instruction_name
            )
        })
}

fn verify_and_extract_idl_data() {
    let idl_json = fs::read(IDL_LOCATION).expect("Failed to read IDL JSON");
    let express_relay_idl =
        convert_idl(idl_json.as_slice()).expect("Failed to convert IDL to Rust");
    println!(
        "cargo:rustc-env=SUBMIT_BID_PERMISSION_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SUBMIT_BID_INSTRUCTION_SVM,
            SUBMIT_BID_PERMISSION_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo:rustc-env=SUBMIT_BID_ROUTER_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SUBMIT_BID_INSTRUCTION_SVM,
            SUBMIT_BID_ROUTER_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_ROUTER_TOKEN_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_ROUTER_TOKEN_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_USER_WALLET_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_USER_WALLET_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_SEARCHER_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_SEARCHER_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_MINT_SEARCHER_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_MINT_SEARCHER_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_MINT_USER_ACCOUNT_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_MINT_USER_ACCOUNT_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_TOKEN_PROGRAM_SEARCHER_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_TOKEN_PROGRAM_SEARCHER_SVM,
        )
    );
    println!(
        "cargo::rustc-env=SWAP_TOKEN_PROGRAM_USER_POSITION={}",
        extract_account_position(
            express_relay_idl.clone(),
            SWAP_INSTRUCTION_SVM,
            SWAP_TOKEN_PROGRAM_USER_SVM,
        )
    );
}

fn build_svm_contracts() {
    let contract_setup_svm = r#"
        pwd
        cd ../contracts/svm/programs/express_relay
        pwd
        mkdir -p ../../target/idl
        pwd
        anchor idl build > ../../target/idl/express_relay.json
    "#;
    println!("cargo:rerun-if-changed=../contracts/svm");
    // Build the svm contract and generate the IDLs.
    let output = Command::new("sh")
        .args(["-c", contract_setup_svm])
        .stdout(Stdio::inherit())
        .output()
        .expect("Failed to run build svm contracts command");
    if !output.status.success() {
        panic!(
            "Failed to build svm contracts: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        println!(
            "Built all svm contracts  {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

fn main() {
    println!("cargo:rerun-if-changed=migrations");

    build_evm_contracts();
    build_svm_contracts();
    verify_and_extract_idl_data();
}
