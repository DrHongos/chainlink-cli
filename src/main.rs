use clap::{Subcommand, Parser};
use ccip::{
    get_chain,
    get_router,
    get_selector,
    get_lane,
    get_status_on_chain,
};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Test {},
    GetRouter {chain_name: String},
    GetSelector {chain_name: String},
//    GetFeeTokens {chain_name: String, selector: u8},
    ChainStatus {
        #[arg(short, long)]
        chain_name: String
    },
    GetLane {
        #[arg(short, long)]
        origin: String,
        #[arg(short, long)]
        destination: String,
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    println!("Chainlink-rs CLI");
    dotenv::dotenv().ok();
//    let rpc_url = dotenv::var("RPC_URL").expect("No RPC_URL found in .env");

    match &args.command {
        Some(Command::Test {}) => {
            println!("Printing test successfully");
        },
        Some(Command::GetRouter { chain_name }) => {
            let chain = get_chain(chain_name).expect("Error with chain selected");
            let router = get_router(&chain).expect("Error looking for router");
            println!("Router for {} is {}", chain_name, format!("{:?}", router));
        },
        Some(Command::GetSelector { chain_name }) => {
            let chain = get_chain(chain_name).expect("Error with chain selected");
            let selector = get_selector(&chain).expect("Error looking for router");
            println!("Selector for {} is {}", chain_name, selector);
        },
        Some(Command::GetLane { origin, destination }) => {
            let chain_s = get_chain(&origin).expect("Error with source");
            let chain_d = get_chain(&destination).expect("Error with destination");
            let lane = get_lane(chain_s, chain_d).expect("Error looking for lane");
            println!("{:#?}", lane);
        },
        Some(Command::ChainStatus { chain_name }) => {
            let chain = get_chain(chain_name).expect("Error with chain selected");
            let pk = dotenv::var("PRIVATE_KEY").expect("No private key supplied to .env");         
            get_status_on_chain(pk, chain).await.expect("Error getting status");
            //println!("{:#?}", user_status);
        },
        _ => println!("Command unknown"),
    }
}
