pub mod constants;
pub mod functions;

use clap::{Subcommand, Parser};
use ccip::{
    get_chain,
    get_router,
    get_selector,
    get_lane,
};
use alloy_providers::provider::{Provider, TempProvider};
use alloy_transport_http::Http;
use alloy_primitives::{Address, utils::format_units, U256};
use reqwest::Client;
use alloy_rpc_client::ClientBuilder;
use constants::get_provider_rpc_url;
use alloy_chains::Chain;
use eyre::Result;
use std::str::FromStr;
//use alloy_primitives::U256;
use datafeeds::{DataFeeds, DataFeedIndexPrices};
use functions::multicall3::Call3;
use alloy_sol_types::{SolCall, SolType};
use alloy_rpc_types::{CallInput, CallRequest};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Test {},
    // Data feeds
    GetChainReferenceFeeds {
        #[arg(short, long)]
        chain: String
    },
    GetOracle {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long)]
        token: String,
        #[arg(short, long)]
        base: String,
    },
    GetLatestAnswer {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long, value_delimiter(','))]
        token: Vec<String>,
        #[arg(short, long, value_delimiter(','))]
        base: Vec<String>,
    },
    GetRoundData {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long)]
        token: String,
        #[arg(short, long)]
        base: String,
        #[arg(short, long, value_delimiter(','))]
        round_id: Vec<u128>,
    },
    GetDescription {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long)]
        token: String,
        #[arg(short, long)]
        base: String,
    },
    // testing for historical price fetching
    GetAllPhases {
        #[arg(short, long)]
        chain: String,
        #[arg(short, long)]
        token: String,
        #[arg(short, long)]
        base: String,
    },    


    // CCIP
    GetRouter {chain_name: String},
    GetSelector {chain_name: String},
//    GetFeeTokens {chain_name: String, selector: u8},
/*     ChainStatus {
        #[arg(short, long)]
        chain_name: String
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
                                                )  
   (       )                   (             ( /(  
   )\   ( /(     )  (          )\ (          )\()) 
 (((_)  )\()) ( /(  )\   (    ((_))\   (   |((_)\  
 )\___ ((_)\  )(_))((_)  )\ )  _ ((_)  )\ )|_ ((_) 
((/ __|| |(_)((_)_  (_) _(_/( | | (_) _(_/(| |/ /  
 | (__ | ' \ / _` | | || ' \))| | | || ' \)) ' <   
  \___||_||_|\__,_| |_||_||_| |_| |_||_||_| _|\_\  
                                                   
    "#);
    dotenv::dotenv().ok();                // get rpc_url
    let rpc_url_id = std::env::var("RPC_URL_ID").expect("No RPC_URL_ID in .env");
    match &args.command {
        Some(Command::Test {}) => {
            println!("Printing test successfully");
        },
        // Data Feeds
        Some(Command::GetChainReferenceFeeds {chain}) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
            println!("{:#?}", price_feeds);
              
        },
        Some(Command::GetOracle { chain, token, base }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
            if let Some(oracle) = price_feeds.get_data_feed_prices(&token.to_uppercase(), &base.to_uppercase()) { 
                println!("{:#?}", oracle)
            } else {
                println!("No oracle found for {token}/{base} in {chain}")
            }
        },
        Some(Command::GetLatestAnswer { chain, token, base }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            if token.len() == 0 || token.len() != base.len() {  // TODO: if base.len() == 1, reuse for all token
                panic!("Wrong input for token/base")
            }
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                match token.len() {
                    1 => functions::datafeeds::get_latest_answer(provider, chain, token.first().unwrap().to_uppercase(), base.first().unwrap().to_uppercase()).await,
                    _ => {
                        let mut tokens: Vec<String> = Vec::new();// token.into_iter().map(|tn| tn.to_uppercase()).collect();
                        let mut bases: Vec<String> =  Vec::new();//base.into_iter().map(|b| b.to_uppercase()).collect();
                        let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
                        let mut all_queries: Vec<Call3> = Vec::new();
                        let mut all_oracles: Vec<&DataFeedIndexPrices> = Vec::new();
                        for (t,b) in token.into_iter().zip(base) {
                            let tt = t.to_uppercase();
                            let bb = b.to_uppercase();
                            if let Some(oracle) = price_feeds.get_data_feed_prices(&tt, &bb) {
                                all_oracles.push(oracle);
                                tokens.push(tt);
                                bases.push(bb);
                                //println!("Creating Call3 with: \ntarget: {}\ncalldata: {:?}", oracle.contract_address.unwrap(), datafeeds::latest_answer_call());
                                all_queries.push(
                                    Call3 {
                                        target: oracle.proxy_address.unwrap(),
                                        allowFailure: true,
                                        callData: datafeeds::latest_answer_call().into_input().unwrap().into()
                                    } 
                                )
                            }
                        }              
                        let mc = functions::multicall3::aggregate3Call {
                            calls: all_queries
                        };
                        let c = CallInput::new(mc.abi_encode().into());                    
                        let tx = CallRequest {
                            to: Some(constants::MULTICALL3.parse::<Address>().unwrap()),
                            input: c,
                            ..Default::default()
                        };
                        //println!("tx {:?}", tx);
                        match provider.call(tx, None).await {
                            Ok(r) => {
                                let results = functions::multicall3::MultiResult::abi_decode(&r, false).unwrap();
                                for (((t,b) , o), r) in tokens.into_iter().zip(bases).zip(all_oracles).zip(results) {
                                    if r.success {
                                        let val = U256::try_from_be_slice(&r.returnData).unwrap_or(U256::ZERO);
                                        println!("Latest answer for {}: {} {}", t, format_units(val, o.decimals.unwrap()).unwrap(), b)
                                    } else {
                                        println!("Error getting latest answer for {}/{}", t, b)
                                    }
                                }
                            },
                            Err(e) => println!("Err {:#?}", e)
                            
                        }
                    }
                }
            }
        },
        // it fails with multiple roundId's when they are old... study! FIX!
        Some(Command::GetRoundData { chain, token, base, round_id }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                match round_id.len() {
                    1 => functions::datafeeds::get_round_data(provider, round_id.first().unwrap().to_owned(), chain, token.to_uppercase(), base.to_uppercase()).await,
                    _ => {
                        let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
                        let mut all_queries: Vec<Call3> = Vec::new();
                        let token = token.to_uppercase();
                        let base = base.to_uppercase();
                        if let Some(oracle) = price_feeds.get_data_feed_prices(&token, &base) {
                            for rid in round_id.into_iter() {
                                let data = datafeeds::get_round_data_call(rid.clone());
                                println!("gonna call {}", oracle.proxy_address.unwrap());
                                println!("with {:?}", data.clone().into_input().unwrap().to_string());
                                all_queries.push(
                                    Call3 {
                                        target: oracle.proxy_address.unwrap(),
                                        allowFailure: true,
                                        callData: data.into_input().unwrap().into()
                                    } 
                                )
                            }
                            let mc = functions::multicall3::aggregate3Call {
                                calls: all_queries
                            };
                            let c = CallInput::new(mc.abi_encode().into());                    
                            let tx = CallRequest {
                                to: Some(constants::MULTICALL3.parse::<Address>().unwrap()),
                                input: c,
                                ..Default::default()
                            };
                            match provider.call(tx, None).await {
                                Ok(r) => {
                                    let results = functions::multicall3::MultiResult::abi_decode(&r, false).unwrap();
                                    for (result, rid) in results.into_iter().zip(round_id) {
                                        if result.success {
                                            let response = functions::datafeeds::GetRoundDataReturn::abi_decode(&result.returnData, false).unwrap();
                                            println!("Get Round data in {} for {}/{} is: \nroundId:{:?} \nanswer: {} [{}]\nstarted at: {}\nupdated at: {}\nanswered id round: {}", 
                                                chain.named().unwrap(), 
                                                token, 
                                                base,
                                                response.roundId, 
                                                response.answer, 
                                                format_units(response.answer, oracle.decimals.unwrap()).unwrap(),
                                                response.startedAt, response.updatedAt, response.answeredInRound
                                            )
                                        } else {
                                            println!("Error getting round data for {}/{} roundId: {}", token, base, rid)
                                        }
                                    }
                                },
                                Err(e) => println!("Err {:#?}", e)
                            }
                        }                        
                    }
                }
            }
        },
        Some(Command::GetDescription { chain, token, base }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                functions::datafeeds::get_description(provider, chain, token.to_uppercase(), base.to_uppercase()).await;
            }
        },
        Some(Command::GetAllPhases { chain, token, base }) => {
            let chain = Chain::from_str(chain).expect("chain not found");
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
                if let Some(oracle) = price_feeds.get_data_feed_prices(&token.to_uppercase(), &base.to_uppercase()) {
                    let all_phases = functions::datafeeds::get_all_phases_addresses(provider, oracle).await;
                    println!("All aggregators for {}/{} in {} are:\n{:#?}", token, base, chain, all_phases);
                }
            }
        },


        // CCIP
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
        /* Some(Command::ChainStatus { chain_name }) => {
            let chain = get_chain(chain_name).expect("Error with chain selected");
            let pk = dotenv::var("PRIVATE_KEY").expect("No private key supplied to .env");         
            get_status_on_chain(pk, chain).await.expect("Error getting status");
            //println!("{:#?}", user_status);
        }, */
        _ => println!("Command unknown"),
    }
}

pub fn get_provider(chain: Chain, rpc_url_id: &str) -> Result<Provider<Http<Client>>> {
// get_provider(chain)
    let rpc_url = get_provider_rpc_url(chain.id(), rpc_url_id).expect("No RPC URL found for {chain}");
    let client = ClientBuilder::default().reqwest_http(rpc_url.parse()?);
    let provider = Provider::new_with_client(client);
    Ok(provider)
}