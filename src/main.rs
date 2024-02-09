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
use reqwest::Client;
use alloy_rpc_client::ClientBuilder;
use constants::get_provider_rpc_url;
use alloy_chains::Chain;
use eyre::Result;
use std::{str::FromStr, sync::Arc};
use datafeeds::OraclesIndex;
use alloy_sol_types::{SolType, SolCall};

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
    Test {},
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
        Some(Command::Test {}) => {
            println!("Call aggregator with lastRoundData");
            let chain = Chain::from_named(alloy_chains::NamedChain::Mainnet);
            let provider = get_provider(chain, &rpc_url_id).unwrap();
            
            let aggregator = "0x37bC7498f4FF12C19678ee8fE19d713b87F6a9e6".parse::<alloy_primitives::Address>().unwrap();
            let aggregator_2 = "0xd3fCD40153E56110e6EEae13E12530e26C9Cb4fd".parse::<alloy_primitives::Address>().unwrap();
            // this works
            
            println!("Calling {} individually", aggregator);
            match functions::datafeeds::get_latest_round_data(provider.clone(), aggregator).await {
                Ok(r) => println!("{:?}", r),
                Err(e) => println!("{:?}", e)
            }
            println!("Calling {} individually", aggregator_2);
            match functions::datafeeds::get_latest_round_data(provider.clone(), aggregator_2).await {
                Ok(r) => println!("{:?}", r),
                Err(e) => println!("{:?}", e)
            }
           
            println!("Calling both with multicall3");
            // now replicate with a multicall
            let mc = functions::multicall3::aggregate3Call {
                calls: vec![
                    functions::multicall3::Call3 {
                        target: aggregator,
                        allowFailure: true,
                        callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::latestRoundDataCall{}.abi_encode().into(),
                    },
                    functions::multicall3::Call3 {
                        target: aggregator_2,
                        allowFailure: true,
                        callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::latestRoundDataCall{}.abi_encode().into(),
                    }
                ]
            };
            let data = alloy_rpc_types::CallInput::new(mc.abi_encode().into());                    
            let tx = alloy_rpc_types::CallRequest {
                to: Some(constants::MULTICALL3.parse::<alloy_primitives::Address>().unwrap()),
                input: data,
                ..Default::default()
            };
            match provider.call(tx, None).await {
                Ok(res) => {
                    let responses = functions::multicall3::MultiResult::abi_decode(&res, false).unwrap();      
                    println!("Got {} responses", responses.len());
                    for response in responses.into_iter() {
                        if response.success {
                            if let Ok(r) = <functions::datafeeds::GetRoundDataReturn as alloy_sol_types::SolValue>::abi_decode(&response.returnData, false) {
                                println!("{:?}", r)
                            } else {
                                println!("Error on result for")
                            }
                        } else {
                            println!("Response w error")
                        }
                    }
                },
                Err(e) => panic!("Error in call {:?}", e)
            }
        },
        
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
        Some(Command::GetAllPhases { args }) => {
            let chain = Chain::from_str(&args.chain).expect("chain not found");
            if let Ok(provider) = get_provider(chain, &rpc_url_id) {
                let price_feeds = OraclesIndex::load_reference_feeds(chain).await;
                if let Some(oracle) = price_feeds.get_oracle(&args.base.to_uppercase(), &args.quote.to_uppercase()) {
                    let all_phases = functions::datafeeds::get_aggregators(provider.clone(), oracle).await;
                    
                    let versions = functions::datafeeds::get_aggregators_version(provider.clone(), all_phases.clone()).await.expect("Couldn't get versions");
                    
                    // versions 2 and below require extra fetching
                    let mut aggs_above_2 = Vec::new();                    
                    
                    // doing this because cannot do multicall on lastRoundData... FIX ME
                    let mut last_round_data = std::collections::HashMap::<alloy_primitives::Address, functions::datafeeds::GetRoundDataReturn>::new();
                    for (a, v) in all_phases.into_iter().zip(versions.clone()) {
                        if v.gt(&alloy_primitives::U256::from(2)) {
                            // THIS WORKS 
                            if let Ok(latest_round_data) = functions::datafeeds::get_latest_round_data(provider.clone(), a.clone()).await {
                                last_round_data.insert(a.clone(), latest_round_data);
                                aggs_above_2.push(a);
                            }
                            //println!("{}:\n{:?}", a, latest_round);
                        }
                    }
                    
                    /*
                        methods failing:
                        getTimestamp 
                        latestTimestamp
                        latestRoundData
                        
                        - multicalls results are all success: false.. 
                        - even for AggregatorContract & AccessControlledAggregator interfaces
                        - using etherscan, some values are ok (and cannot retrieve it here)
                    */
                    // this method fails.. still idk why
                    //let time_starts = functions::datafeeds::find_addresses_last_round_data(provider.clone(), aggs_above_2.clone()).await;
                    //println!("{:?}", time_starts);
/*                     
                    for agg in aggs_above_2.clone().into_iter() {
                        println!("Aggregator {} last block data\n{:?}", agg, last_round_data.get(&agg).unwrap());
                    }
 */                    
                    // testing

                    // perform a search of historical data in each aggregator, using get_multiple_round_data()
                    // check the failing cases (hipothesis: only roundIds in the aggregator..)

                    let last_round_proxy_round_id = functions::datafeeds::get_latest_round_data(provider.clone(), oracle.proxy_address.unwrap()).await.unwrap().roundId;                    
                    // lets get some roundData's
                    let current_aggregator_address = aggs_above_2.last().unwrap(); 
                    let current_aggregator_last_round_id = last_round_data.get(current_aggregator_address).unwrap().roundId;
                    let from_last_aggregator = last_round_proxy_round_id - current_aggregator_last_round_id;
                    println!("Total roundId is {} and {} are from the current aggregator", last_round_proxy_round_id, current_aggregator_last_round_id);
                    println!("Create a vector from {} to {}", from_last_aggregator, last_round_proxy_round_id);
                    
                    // take a few
                    //let round_id_acum: Vec<u128> = (from_last_aggregator+10..last_round_proxy_round_id).collect();       
                    //let shorted_list_round_ids = round_id_acum.into_iter().take(10).collect();
                    
                    // last ten round ids
                    let shorted_list_round_ids: Vec<u128> = (last_round_proxy_round_id-10..last_round_proxy_round_id).collect();       
                    println!("Aggregator address {}", current_aggregator_address);
                    let res = functions::datafeeds::get_multiple_round_data(
                        provider, 
                        oracle.proxy_address.unwrap(),
                    //    current_aggregator_address.clone(),   // should be this one.. but it fails 
                        shorted_list_round_ids
                    ).await;
                    if let Ok(res) = res {
                        for r in res {
                            println!("{:?}", r);
                        }                    
                    }    
                    // it fails! seems like round_id's aren't trivial... AGAIN
                }
            }
        },
        //------------------------------------------------------------------------------//
        // CCIP
        Some(Command::GetRouter { chain_name }) => {
            let chain = get_chain(chain_name).expect("Error with chain selected");
            let router = get_router(&chain).expect("Error looking for router");
            println!("Router for {} is {}", chain_name, format!("{}", router));
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

pub fn get_provider(chain: Chain, rpc_url_id: &str) -> Result<Arc<Provider<Http<Client>>>> {
// get_provider(chain)
    let rpc_url = get_provider_rpc_url(chain.id(), rpc_url_id).expect("No RPC URL found for {chain}");
    let client = ClientBuilder::default().reqwest_http(rpc_url.parse()?);
    let provider = Provider::new_with_client(client);
    Ok(Arc::new(provider))
}