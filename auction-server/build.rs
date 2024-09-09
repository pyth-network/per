use {
    anchor_lang_idl::{
        convert::convert_idl,
        types::IdlInstructionAccountItem,
    },
    std::{
        fs,
        process::Command,
    },
};

const SUBMIT_BID_INSTRUCTION_SVM: &str = "submit_bid";
const PERMISSION_ACCOUNT_SVM: &str = "permission";

fn verify_idl() {
    let idl_json = fs::read("../contracts/svm/target/idl/express_relay.json")
        .expect("Failed to read IDL JSON");
    let express_relay_idl =
        convert_idl(idl_json.as_slice()).expect("Failed to convert IDL to Rust");
    match express_relay_idl
        .instructions
        .iter()
        .find(|i| i.name == SUBMIT_BID_INSTRUCTION_SVM)
    {
        Some(instruction) => {
            if !instruction.accounts.iter().any(|a| match a {
                IdlInstructionAccountItem::Single(a) => a.name == PERMISSION_ACCOUNT_SVM,
                IdlInstructionAccountItem::Composite(a) => a.name == PERMISSION_ACCOUNT_SVM,
            }) {
                panic!(
                    "{} account not found in {} instruction",
                    PERMISSION_ACCOUNT_SVM, SUBMIT_BID_INSTRUCTION_SVM
                );
            }
        }
        None => panic!(
            "{} instruction not found in IDL",
            SUBMIT_BID_INSTRUCTION_SVM
        ),
    }
}

fn main() {
    let contract_setup = r#"
        cd ../contracts/evm
        forge build --via-ir
    "#;
    println!("cargo:rerun-if-changed=../contracts/evm");
    println!("cargo:rerun-if-changed=migrations");

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

    verify_idl();
}
