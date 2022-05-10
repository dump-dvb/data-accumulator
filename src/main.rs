extern crate clap;

mod structs;
mod processor;
mod stations;
mod storage;

use structs::{Response, Args, StopConfig};
use processor::{Processor, Telegram, RawData, DEPULICATION_BUFFER_SIZE};
use stations::{Station};
pub use storage::{SaveTelegram, Storage, InfluxDB};

use dvb_dump::receives_telegrams_client::{ReceivesTelegramsClient};
use dvb_dump::{ ReducedTelegram };

pub mod dvb_dump{
    tonic::include_proto!("dvbdump");
}

use tonic::transport::Endpoint;

use actix_web::{web, App, HttpServer, Responder, HttpRequest};
use std::env;
use std::sync::{RwLock};
use clap::Parser;
use std::collections::HashMap;
use std::u32;

async fn formatted(processor: web::Data<RwLock<Processor>>,
                   telegram: web::Json<Telegram>, 
                   req: HttpRequest) -> impl Responder {

    let telegram_hash = Processor::calculate_hash(&telegram).await;
    let contained;
    // checks if the given telegram is already in the buffer
     {
        let readable_processor = processor.read().unwrap();
        contained = readable_processor.last_elements.contains(&telegram_hash);
    }

    // updates the buffer adding the new telegram
    {
        let mut writeable_processor = processor.write().unwrap();
        let index = writeable_processor.iterator;
        writeable_processor.last_elements[index] = telegram_hash;
        writeable_processor.iterator = (writeable_processor.iterator + 1) % DEPULICATION_BUFFER_SIZE;
    }

    let ip_address;
    if let Some(val) = req.peer_addr() {
        ip_address = val.ip().to_string();
        println!("Address {:?}", val.ip());
    } else {
        return web::Json(Response { success: false });
    }

    // do more processing
    if !contained {
        let default_public_api = String::from("http://127.0.0.1:50051");
        let url_public_api = Endpoint::from_shared(env::var("PUBLIC_API").unwrap_or(default_public_api)).unwrap();

        //let default_public_api = String::from("../stops.json");
        //let url_public_api = env::var("STOPS_CONFIG").unwrap_or(default_public_api);
        let save = SaveTelegram::from(&*telegram, &ip_address);
        {
            let mut writeable_processor = processor.write().unwrap();
            writeable_processor.write(save).await;

        }

        const FILE_STR: &str = include_str!("../stops.json");
        let parsed: HashMap<String, StopConfig> = serde_json::from_str(FILE_STR).expect("JSON was not well-formatted");

        let lat;
        let lon;
        let station_name;

        // dont cry code reader this will TM be replaced by postgress look up 
        // revol-xut May the 8 2022
        let stations = HashMap::from([
            (String::from("10.13.37.100"), Station {
                name: String::from("Barkhausen/Turmlabor"),
                lat: 51.026107,
                lon: 13.623566,
                station_id: 0,
                region_id: 0  
            }),
            (String::from("10.13.37.101"), Station {
                name: String::from("Zentralwerk"),
                lat: 51.0810632,
                lon: 13.7280758,
                station_id: 1,
                region_id: 0,
            }),
            (String::from("10.13.37.102"), Station {
                name: String::from(""),
                lat: 51.0810632,
                lon: 13.7280758,
                station_id: 2,
                region_id: 1 
            }),
        ]);

        match parsed.get(&telegram.junction.to_string()) {
            Some(data) => {
                lat = data.lat;
                lon = data.lon;
                station_name = data.name.clone();
            }
            None => {
                lat = 0f64;
                lon = 0f64;
                station_name = String::from("");
            }
        }

        let region_code = match stations.get(&ip_address) {
            Some(station) => {
                station.region_id
            }
            None => {
                return web::Json(Response { success: false });
            }
        };

        let request = tonic::Request::new(ReducedTelegram {
            time_stamp: telegram.time_stamp,
            position_id: telegram.junction,
            direction: telegram.request_for_priority,
            status: telegram.request_status,
            line: telegram.line.parse::<u32>().unwrap_or(0),
            delay: ((telegram.sign_of_deviation as i32) * 2 - 1) * telegram.value_of_deviation as i32,
            destination_number: telegram.destination_number.parse::<u32>().unwrap_or(0),
            run_number: telegram.run_number.parse::<u32>().unwrap_or(0),
            lat: lat as f32,
            lon: lon as f32,
            station_name,
            train_length: telegram.train_length,
            region_code
        });

        match ReceivesTelegramsClient::connect(url_public_api).await {
            Ok(mut client) => {
                client.receive_new(request).await;
            }
            Err(_) => {
                println!("Cannot connect to GRPC Host");
            }
        };
    }

    web::Json(Response { success: true })
}

async fn raw(telegram: web::Json<RawData>) -> impl Responder {
    //let default_file = String::from("/var/lib/data-accumulator/raw_data.csv");
    //let csv_file = env::var("PATH_RAW_DATA").unwrap_or(default_file);

    println!("Received Formatted Record: {:?}", &telegram);
    //Processor::dump_to_file(&csv_file, &telegram).await;

    web::Json(Response { success: true })
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    println!("Starting Data Collection Server ... ");
    let host = args.host.as_str();
    let port = args.port;

    println!("Listening on: {}:{}", host, port);
    let data = web::Data::new(RwLock::new(Processor::new())); 
    HttpServer::new(move || App::new()
                    .app_data(data.clone())
                    .route("/formatted_telegram", web::post().to(formatted))
                    .route("/raw_telegram", web::post().to(raw))

                    )
        .bind((host, port))?
        .run()
        .await
}


