use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};

pub fn parse_config() -> HashMap<String, String> {
    let mut config: HashMap<String, String> = HashMap::new();
    let file = File::open("/home/bobby/code/apps/rush/config.yaml").expect("Unable to read config file: Does not exist");
    let reader = io::BufReader::new(file);
    for line in reader.lines() {
        let line = line.expect("Could not read line");
        let settings: Vec<&str> = line.split(":").collect();
        if settings.len() < 2 { continue };
        println!("{:?}", settings);
        config.insert(
            settings[0].trim().to_string(),
            settings[1].trim().to_string()
        );
    }

    config
}
