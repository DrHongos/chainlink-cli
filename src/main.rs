pub mod constants;
pub mod functions;

use clap::{Subcommand, Parser};
use ccip::{
    get_chain,
    get_router,
    get_selector,
    get_lane,
};
use alloy_providers::provider::Provider;
use alloy_transport_http::Http;
use reqwest::Client;
use alloy_rpc_client::ClientBuilder;
use constants::get_provider_rpc_url;
use alloy_chains::Chain;
use eyre::Result;
use std::{str::FromStr, sync::Arc};
use datafeeds::OraclesIndex;

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Parser)] 
pub struct PairSetting {
    #[clap(short, long, value_name = "chain name", required = true)]
    pub chain: String,
    #[clap(short, long, value_name = "base", required = true)]
    pub base: String,
    #[clap(short, long, value_name = "quote", required = true)]
    pub quote: String,
}
// make a similar object for multiple inputs? (adds delimiter)

#[derive(Debug, Subcommand)]
enum Command {
    // Data feeds
    GetOracle {
        #[clap(flatten)]
        args: PairSetting,
    },
    GetLatestAnswer {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long, value_delimiter(','))]
        base: Vec<String>,
        #[arg(short, long, value_delimiter(','))]
        quote: Vec<String>,
    },
    GetLatestRoundData {
        #[clap(flatten)]
        args: PairSetting,
    },
    GetRoundData {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long)]
        base: String,
        #[arg(short, long)]
        quote: String,
        #[arg(short, long, value_delimiter(','))]
        round_id: Vec<u128>,
    },
    GetDescription {
        #[clap(flatten)]
        args: PairSetting,
    },
    // testing for historical price fetching
    GetAllPhases {
        #[clap(flatten)]
        args: PairSetting,
    },    


    // CCIP
    GetRouter {chain: String},
    GetSelector {chain: String},
//    GetFeeTokens {chain: String, selector: u8},
/*     ChainStatus {
        #[arg(short, long)]
        chain: String
    }, */
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
    println!(r#"
 _____ _           _       _     _____      _    
/  __ \ |         (_)     | |   |_   _|    | |   
| /  \/ |__   __ _ _ _ __ | |     | | _ __ | | __
| |   | '_ \ / _` | | '_ \| |     | || '_ \| |/ /
| \__/\ | | | (_| | | | | | |_____| || | | |   < 
 \____/_| |_|\__,_|_|_| |_\_____/\___/_| |_|_|\_\
                                                 
    "#);
    dotenv::dotenv().ok();
    let rpc_url_id = std::env::var("RPC_URL_ID").expect("No RPC_URL_ID in .env");
    match &args.command {
        
        // Data Feeds
        Some(Command::GetOracle { args }) => {
            let chain = Chain::from_str(&args.chain).expect("chain not found");
            let datafeeds = OraclesIndex::load_reference_feeds(chain).await;
            if let Some(oracle) = datafeeds.get_oracle(&args.base.to_uppercase(), &args.quote.to_uppercase()) { 
                println!("{:#?}", oracle)
            } else {
                println!("No oracle found for {}/{} in {chain}", args.base, args.quote)
            }
        },
        Some(Command::GetLatestAnswer { chain, base, quote }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            if base.len() == 0 || base.len() != quote.len() {  // TODO: if quote.len() == 1, reuse for all bases
                panic!("Wrong input for token/base")
            }
            let datafeeds = OraclesIndex::load_reference_feeds(chain).await;
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                match base.len() {
                    1 => {
                        let base = base.first().unwrap().to_uppercase();
                        let quote = quote.first().unwrap().to_uppercase();
                        if let Some(oracle) = datafeeds.get_oracle(&base, &quote) {
                        let r = functions::datafeeds::get_latest_answer(provider, oracle.proxy_address.unwrap()).await.unwrap();
                        println!("{}/{} in [{}] is {} [{}]",
                            base, quote, chain, r,
                            alloy_primitives::utils::format_units(r, oracle.decimals.unwrap()).unwrap()
                        );
                        }
                    },
                    _ => {
                        let _ = functions::datafeeds::get_multiple_latest_answer(provider, chain, base.to_vec(), quote.to_vec()).await;
                    },
                }
            }
        },
        Some(Command::GetLatestRoundData { args }) => {
            let chain = Chain::from_str(&args.chain).expect(format!("chain not found for {}", args.chain).as_ref());
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                let price_feeds = OraclesIndex::load_reference_feeds(chain).await;
                if let Some(oracle) = price_feeds.get_oracle(&args.base.to_uppercase(), &args.quote.to_uppercase()) {
                    let oracle_address= oracle.proxy_address.expect("This settings has no oracle defined");
                    if let Ok(res) = functions::datafeeds::get_latest_round_data(provider, oracle_address).await {
                        println!("{:?}", res)
                    } else {
                        println!("Error getting last round data for {}", oracle_address)
                    }
                }
            }
        },
        Some(Command::GetDescription { args }) => {
            let chain = Chain::from_str(&args.chain).expect("chain not found");
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                let datafeeds = OraclesIndex::load_reference_feeds(chain).await;
                if let Some(oracle) = datafeeds.get_oracle(&args.base.to_uppercase(), &args.quote.to_uppercase()) {                    
                    functions::datafeeds::get_description(provider, oracle).await;
                }
            }
        },

        // seems like multicall to the same Aggregator works fine, but mixed isn't
        Some(Command::GetRoundData { chain, base, quote, round_id }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            let base = base.to_uppercase();
            let quote = quote.to_uppercase();
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                let price_feeds = OraclesIndex::load_reference_feeds(chain).await;
                if let Some(oracle) = price_feeds.get_oracle(&base, &quote) {
                    match round_id.len() {
                        1 => {
                            let r = functions::datafeeds::get_round_data(provider, oracle.proxy_address.unwrap(), round_id.first().unwrap().to_owned()).await;
                            println!("Round data for {}/{} [{}] in round-id {} \n{:?}",
                                base, quote, chain, round_id.first().unwrap(), r
                            );
                        },
                        _ => {
                            let res = functions::datafeeds::get_multiple_round_data(provider, oracle.proxy_address.unwrap(), round_id.to_vec()).await;
                            if let Ok(res) = res {
                                for r in res {
                                    println!("{:?}", r);
                                }

                            }
                        },
                    }
                }
            }
        },
        //------------------------------------------------------------------------------//
        // CCIP
        Some(Command::GetRouter { chain }) => {
            let chain = get_chain(chain).expect("Error with chain selected");
            let router = get_router(&chain).expect("Error looking for router");
            println!("Router for {} is {}", chain, format!("{}", router));
        },
        Some(Command::GetSelector { chain }) => {
            let chain = get_chain(chain).expect("Error with chain selected");
            let selector = get_selector(&chain).expect("Error looking for router");
            println!("Selector for {} is {}", chain, selector);
        },
        Some(Command::GetLane { origin, destination }) => {
            let chain_s = get_chain(&origin).expect("Error with source");
            let chain_d = get_chain(&destination).expect("Error with destination");
            let lane = get_lane(chain_s, chain_d).expect("Error looking for lane");
            println!("{:#?}", lane);
        },
        /* Some(Command::ChainStatus { chain_name }) => {
            let chain = get_chain(chain_name).expect("Error with chain selected");
            let pk = dotenv::var("PRIVATE_KEY").expect("No private key supplied to .env");         
            get_status_on_chain(pk, chain).await.expect("Error getting status");
            //println!("{:#?}", user_status);
        }, */
        _ => println!("Command unknown"),
    }
}

pub fn get_provider(chain: Chain, rpc_url_id: &str) -> Result<Arc<Provider<Http<Client>>>> {
// get_provider(chain)
    let rpc_url = get_provider_rpc_url(chain.id(), rpc_url_id).expect("No RPC URL found for {chain}");
    let client = ClientBuilder::default().reqwest_http(rpc_url.parse()?);
    let provider = Provider::new_with_client(client);
    Ok(Arc::new(provider))
}