use std::env;
use std::error::Error;
use  chrono::{NaiveTime, Utc};



use anime_reflink::config::Config;
use anime_reflink::data::Data;

fn main() -> Result<(), Box<dyn Error>> {
    let start_time: NaiveTime = Utc::now().time();

    let config = Config::new(env::args());

    println!("Run for {}", config.action);
    println!("In file {}", config.mapfile_path);
    println!("In source {}", config.source_path);
    println!("In anime {}", config.anime_path);

    let mut data = Data::from_yaml(config);
    data.push_map_from_dir();
    data.push_anime_from_dir()?;
    data.map_animes()?;

    data.write_yaml()?;

   
    let end_time: NaiveTime = Utc::now().time();
    let diff = end_time - start_time;
    println!("Total time taken to run is {}", diff);
    Ok(())
}
