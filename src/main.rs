use dirs_next::home_dir;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use rusoto_core::{HttpClient, Region, RusotoError};
use rusoto_credential::StaticProvider;
use rusoto_ec2::{
    DescribeInstancesError, DescribeInstancesRequest, DescribeInstancesResult, Ec2, Ec2Client,
    Instance, Reservation,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

struct InstanceMetadata {
    name: String,
    ip: String,
    region: Region,
}

#[tokio::main]
async fn main() {
    let creds_file_lines = read_credentials_file();
    let creds = match creds_file_lines {
        Ok(lines) => aws_creds_list(lines),
        Err(_) => {
            eprintln!("Error reading credentials file.");
            std::process::exit(1);
        }
    };

    let regions = vec![
        Region::ApEast1,
        Region::ApNortheast1,
        Region::ApNortheast2,
        Region::ApNortheast3,
        Region::ApSouth1,
        Region::ApSoutheast1,
        Region::ApSoutheast2,
        Region::CaCentral1,
        Region::EuCentral1,
        Region::EuWest1,
        Region::EuWest2,
        Region::EuWest3,
        Region::EuNorth1,
        Region::MeSouth1,
        Region::SaEast1,
        Region::UsEast1,
        Region::UsEast2,
        Region::UsWest1,
        Region::UsWest2,
        // Region::UsGovEast1,
        // Region::UsGovWest1,
        // Region::CnNorth1,
        // Region::CnNorthwest1,
    ];

    let mut futs = FuturesUnordered::new();
    for cred in creds {
        for region in &regions {
            futs.push(regional_instances(region.to_owned(), cred.clone()))
        }
    }

    while let Some(instances) = futs.next().await {
        match instances {
            Ok(instances) => {
                for instance in instances {
                    println!(
                        "{} - {} ({})",
                        instance.name,
                        instance.ip,
                        instance.region.name()
                    );
                }
            }
            Err(_) => { /* Ignore error with region */ }
        }
    }
}

fn hardcoded_profile_location() -> PathBuf {
    match home_dir() {
        Some(mut home_path) => {
            home_path.push(".aws");
            home_path.push("credentials");
            home_path
        }
        None => {
            eprintln!("Failed to determine home directory.");
            std::process::exit(1);
        }
    }
}

fn read_credentials_file() -> std::io::Result<Vec<String>> {
    let file_path = hardcoded_profile_location();
    let file = File::open(file_path.as_path())?;
    let buf_reader = BufReader::new(file);
    let lines = buf_reader
        .lines()
        .map(|line| line.unwrap_or(String::from("")))
        .collect();
    Ok(lines)
}

fn aws_creds_list(lines: Vec<String>) -> Vec<StaticProvider> {
    let mut key = None;
    let mut secret = None;
    let mut creds = Vec::new();
    for line in lines {
        let mut words = line.split("=");
        match words.next() {
            Some(word) => match word.trim() {
                "aws_access_key_id" => {
                    key = Some(words.next().unwrap().trim().to_string());
                }
                "aws_secret_access_key" => {
                    secret = Some(words.next().unwrap().trim().to_string());
                }
                _ => {}
            },
            None => {}
        }

        match (key.take(), secret.take()) {
            (Some(key_value), Some(secret_value)) => {
                creds.push(StaticProvider::new(key_value, secret_value, None, None));
            }
            (taken_key, taken_secret) => {
                key = taken_key;
                secret = taken_secret;
            }
        }
    }
    creds
}

fn instances_result(
    output: Result<DescribeInstancesResult, RusotoError<DescribeInstancesError>>,
    region: &Region,
) -> Result<DescribeInstancesResult, String> {
    match output {
        Ok(result) => Ok(result),
        Err(_) => Err(format!("Region failure in {}", &region.name())),
    }
}

fn instances_reservations(result: DescribeInstancesResult) -> Result<Vec<Reservation>, String> {
    match result.reservations {
        Some(reservations) => Ok(reservations),
        None => Err(String::from("No reservations")),
    }
}

fn instance_name(instance: &Instance) -> Result<String, String> {
    match &instance.tags {
        Some(tags) => {
            for tag in tags {
                match &tag.key {
                    Some(key) => {
                        if key.as_str() == "Name" {
                            match &tag.value {
                                Some(value) => {
                                    return Ok(value.to_owned());
                                }
                                None => (),
                            }
                        }
                    }
                    None => (),
                }
            }
            Err(String::new())
        }
        None => Err(String::new()),
    }
}

async fn regional_instances(
    region: Region,
    credential: StaticProvider,
) -> Result<Vec<InstanceMetadata>, String> {
    let client = Ec2Client::new_with(HttpClient::new().unwrap(), credential, region.clone());
    let describe_instances_input: DescribeInstancesRequest = DescribeInstancesRequest {
        dry_run: None,
        filters: None,
        instance_ids: None,
        max_results: None,
        next_token: None,
    };

    let result = instances_result(
        client.describe_instances(describe_instances_input).await,
        &region,
    )?;

    let reservations = instances_reservations(result)?;

    let total_instances: Vec<Instance> = reservations
        .into_iter()
        .filter_map(|reservation| reservation.instances)
        .flatten()
        .collect();

    let mut instance_metadatas = Vec::new();

    for instance in total_instances {
        let ip_address = match &instance.public_ip_address {
            Some(ip_address) => ip_address,
            None => "N/A",
        };

        let name = match instance_name(&instance) {
            Ok(name) => name,
            Err(_) => String::from("N/A"),
        };

        instance_metadatas.push(InstanceMetadata {
            name,
            ip: String::from(ip_address),
            region: region.clone(),
        });
    }

    Ok(instance_metadatas)
}
