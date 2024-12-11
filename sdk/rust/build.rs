use std::{
    env,
    process::Command,
};

fn build_evm_contracts() {
    let mut current_dir = env::current_dir().expect("Failed to get current directory");
    loop {
        if current_dir.join("contracts").exists() {
            current_dir = current_dir.join("contracts/evm");
            break;
        }
        current_dir = current_dir
            .parent()
            .expect("Failed to find contracts directory")
            .into();
    }

    let abis = [
        "OpportunityAdapter",
        "OpportunityAdapterFactory",
        "ERC20",
        "WETH9",
        "ExpressRelay",
    ];

    let contract_setup = format!(
        r#"
        cd {}
        forge build --via-ir
        cd ../../sdk/rust
        mkdir -p abi
    "#,
        current_dir.to_str().unwrap()
    );

    // Generate `cp` commands for each ABI
    let copy_commands: String = abis
        .iter()
        .map(|abi| format!("cp -r ../../contracts/evm/out/{}.sol ./abi/.", abi))
        .collect::<Vec<_>>()
        .join("\n");

    let full_script = format!("{contract_setup}\n{copy_commands}");
    println!(
        "cargo:rerun-if-changed={}/evm",
        current_dir.to_str().unwrap()
    );
    // Build the contracts and generate the ABIs. This is required for abigen! macro expansions to work.
    let output = Command::new("sh")
        .args(["-c", full_script.as_str()])
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

fn main() {
    build_evm_contracts();
}
