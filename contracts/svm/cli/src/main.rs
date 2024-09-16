use {
    anchor_client::{
        anchor_lang::prelude::Pubkey,
        solana_sdk::{
            commitment_config::CommitmentConfig,
            signature::{
                read_keypair_file,
                Keypair,
            },
            signer::Signer,
            system_program,
        },
        Client,
        Cluster,
    },
    clap::{
        Parser,
        Subcommand,
    },
    express_relay::{
        accounts,
        instruction,
        state::{
            ConfigRouter,
            ExpressRelayMetadata,
            SEED_CONFIG_ROUTER,
            SEED_METADATA,
        },
        InitializeArgs,
        SetRouterSplitArgs,
        SetSplitsArgs,
    },
    std::str::FromStr,
};


/// CLI utility for interacting with Express Relay SVM program
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "http://127.0.0.1:8899")]
    /// RPC URL of the solana cluster
    rpc_url: String,

    #[arg(long)]
    /// Override the program id
    program_id: Option<Pubkey>,

    #[command(subcommand)]
    command: Commands,
}


#[derive(Debug, Parser)]
struct Initialize {
    #[arg(long)]
    /// Path to the private key json file for the payer of the initialization transaction
    pub payer: String,

    #[arg(long)]
    /// Program admin, defaults to payer
    pub admin: Option<Pubkey>,

    #[arg(long)]
    /// signer used for relaying the bids, defaults to payer
    pub relayer_signer: Option<Pubkey>,

    #[arg(long)]
    /// Fee receiver for the relayer, defaults to payer
    pub fee_receiver_relayer: Option<Pubkey>,

    #[arg(long, default_value = "4000")]
    /// The portion of the bid that goes to the router, in bps
    pub split_router_default: u64,

    #[arg(long, default_value = "2000")]
    /// The portion of the bid (after router fees) that goes to relayer, in bps
    pub split_relayer: u64,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Initialize(Initialize),
    SetAdmin {
        #[arg(long)]
        /// Path to the private key json file for the current admin
        /// This account will be used as the transaction payer as well
        admin: String,

        #[arg(long)]
        /// New admin pubkey
        admin_new: Pubkey,
    },
    SetRelayer {
        #[arg(long)]
        /// Path to the private key json file for the admin
        /// This account will be used as the transaction payer as well
        admin: String,

        #[arg(long)]
        /// Signer used for relaying the bids
        relayer_signer: Pubkey,

        #[arg(long)]
        /// Fee receiver for the relayer
        fee_receiver_relayer: Pubkey,
    },
    SetSplits {
        #[arg(long)]
        /// Path to the private key json file for the admin
        /// This account will be used as the transaction payer as well
        admin: String,

        #[arg(long)]
        /// The portion of the bid that goes to the router, in bps
        split_router_default: u64,

        #[arg(long)]
        /// The portion of the bid (after router fees) that goes to relayer, in bps
        split_relayer: u64,
    },
    SetRouterSplit {
        #[arg(long)]
        /// Path to the private key json file for the admin
        /// This account will be used as the transaction payer as well
        admin: String,

        #[arg(long)]
        /// The pubkey of the router to set the split for
        router: Pubkey,

        #[arg(long)]
        /// The split to use for this specific router, in bps
        split_router: u64,
    },
    WithdrawFees {
        #[arg(long)]
        /// Path to the private key json file for the admin
        /// This account will be used as the transaction payer as well
        admin: String,

        #[arg(long)]
        /// The pubkey of the account that receives the fees
        fee_receiver: Pubkey,
    },
    GetConfig,
    GetRouterSplit {
        #[arg(long)]
        /// The pubkey of the router to get the split for
        router: Pubkey,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let program_id = args.program_id.unwrap_or(express_relay::ID);
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &program_id).0;
    let cluster = Cluster::from_str(args.rpc_url.as_str())?;

    match args.command {
        Commands::Initialize(init_args) => {
            let payer = read_keypair_file(init_args.payer)?;
            let client = Client::new_with_options(cluster, &payer, CommitmentConfig::confirmed());
            let program = client.program(program_id)?;

            let result = program
                .request()
                .accounts(accounts::Initialize {
                    payer: payer.pubkey(),
                    express_relay_metadata,
                    admin: init_args.admin.unwrap_or(payer.pubkey()),
                    relayer_signer: init_args.relayer_signer.unwrap_or(payer.pubkey()),
                    fee_receiver_relayer: init_args.fee_receiver_relayer.unwrap_or(payer.pubkey()),
                    system_program: system_program::ID,
                })
                .args(instruction::Initialize {
                    data: InitializeArgs {
                        split_router_default: init_args.split_router_default,
                        split_relayer:        init_args.split_relayer,
                    },
                })
                .signer(&payer)
                .send()?;
            println!(
                "Initialized express relay program with metadata account {:?}",
                express_relay_metadata
            );
            println!("Transaction signature {:?}", result);
        }
        Commands::SetAdmin { admin, admin_new } => {
            let payer = read_keypair_file(admin)?;
            let client = Client::new_with_options(cluster, &payer, CommitmentConfig::confirmed());
            let program = client.program(program_id)?;

            let result = program
                .request()
                .accounts(accounts::SetAdmin {
                    express_relay_metadata,
                    admin: payer.pubkey(),
                    admin_new,
                })
                .args(instruction::SetAdmin {})
                .signer(&payer)
                .send()?;
            println!("Set admin to {:?}", admin_new);
            println!("Transaction signature {:?}", result);
        }
        Commands::SetRelayer {
            admin,
            relayer_signer,
            fee_receiver_relayer,
        } => {
            let payer = read_keypair_file(admin)?;
            let client = Client::new_with_options(cluster, &payer, CommitmentConfig::confirmed());
            let program = client.program(program_id)?;

            let result = program
                .request()
                .accounts(accounts::SetRelayer {
                    admin: payer.pubkey(),
                    express_relay_metadata,
                    relayer_signer,
                    fee_receiver_relayer,
                })
                .args(instruction::SetRelayer {})
                .signer(&payer)
                .send()?;
            println!(
                "Set relayer signer to {:?} and fee receiver to {:?}",
                relayer_signer, fee_receiver_relayer
            );
            println!("Transaction signature {:?}", result);
        }
        Commands::SetSplits {
            admin,
            split_router_default,
            split_relayer,
        } => {
            let payer = read_keypair_file(admin)?;
            let client = Client::new_with_options(cluster, &payer, CommitmentConfig::confirmed());
            let program = client.program(program_id)?;

            let result = program
                .request()
                .accounts(accounts::SetSplits {
                    admin: payer.pubkey(),
                    express_relay_metadata,
                })
                .args(instruction::SetSplits {
                    data: SetSplitsArgs {
                        split_relayer,
                        split_router_default,
                    },
                })
                .signer(&payer)
                .send()?;
            println!(
                "Set default router split to {:?} and relayer to {:?} bps",
                split_router_default, split_relayer
            );
            println!("Transaction signature {:?}", result);
        }
        Commands::SetRouterSplit {
            admin,
            router,
            split_router,
        } => {
            let payer = read_keypair_file(admin)?;
            let client = Client::new_with_options(cluster, &payer, CommitmentConfig::confirmed());
            let program = client.program(program_id)?;

            let config_router = get_router_config_account(&program_id, &router);

            let result = program
                .request()
                .accounts(accounts::SetRouterSplit {
                    admin: payer.pubkey(),
                    config_router,
                    express_relay_metadata,
                    router,
                    system_program: system_program::ID,
                })
                .args(instruction::SetRouterSplit {
                    data: SetRouterSplitArgs { split_router },
                })
                .signer(&payer)
                .send()?;
            println!("Set router {:?} split to {:?} bps", router, split_router);
            println!("Transaction signature {:?}", result);
        }
        Commands::WithdrawFees {
            admin,
            fee_receiver,
        } => {
            let payer = read_keypair_file(admin)?;
            let client = Client::new_with_options(cluster, &payer, CommitmentConfig::confirmed());
            let program = client.program(program_id)?;

            let result = program
                .request()
                .accounts(accounts::WithdrawFees {
                    admin: payer.pubkey(),
                    fee_receiver_admin: fee_receiver,
                    express_relay_metadata,
                })
                .args(instruction::WithdrawFees {})
                .signer(&payer)
                .send()?;
            println!("Withdrew fees to {:?}", fee_receiver);
            println!("Transaction signature {:?}", result);
        }
        Commands::GetConfig => {
            let keypair = Keypair::new();
            let client = Client::new(cluster, &keypair);
            let program = client.program(program_id)?;
            let metadata: ExpressRelayMetadata = program.account(express_relay_metadata)?;
            println!("Admin {:?}", metadata.admin);
            println!("Relayer signer {:?}", metadata.relayer_signer);
            println!("Fee receiver relayer {:?}", metadata.fee_receiver_relayer);
            println!("Split router default {:?}", metadata.split_router_default);
            println!("Split relayer {:?}", metadata.split_relayer);
        }
        Commands::GetRouterSplit { router } => {
            let keypair = Keypair::new();
            let client = Client::new(cluster, &keypair);
            let program = client.program(program_id)?;
            let config_router = get_router_config_account(&program_id, &router);
            let config_router_account: ConfigRouter = program.account(config_router)?;
            println!(
                "Router {:?} split {:?}",
                router, config_router_account.split
            );
        }
    };

    Ok(())
}

fn get_router_config_account(program_id: &Pubkey, router: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[SEED_CONFIG_ROUTER, router.as_ref()], program_id).0
}
