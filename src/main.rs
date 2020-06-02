use clap::{App, Arg};
use colored::*;
use dirs_next::home_dir;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::FuturesOrdered;
use futures::stream::StreamExt;
use rusoto_core::{HttpClient, Region};
use rusoto_credential::StaticProvider;
use rusoto_ec2::{DescribeInstancesRequest, DescribeRegionsRequest, Ec2, Ec2Client, Instance};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::str::FromStr;

struct NamedStaticProvider {
    name: String,
    provider: StaticProvider,
}

struct RegionalClient {
    client: Ec2Client,
    region: Region,
    profile: String,
}

struct InstanceMetadata {
    name: String,
    ip: String,
}

struct InstanceMetadataCollection {
    metadatas: Vec<InstanceMetadata>,
    profile: String,
    region: Region,
}

#[tokio::main]
async fn main() {
    // Create CLI system
    let matches = App::new("drips")
        .version("0.1.0")
        .author("Noah Masur <noah.masur@take2games.com>")
        .about("Retrieves AWS EC2 IPs")
        .arg(
            Arg::with_name("region")
                .short("r")
                .long("region")
                .value_name("REGION NAME")
                .help("Filter to a specific region")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("profile")
                .short("p")
                .long("profile")
                .value_name("PROFILE NAME")
                .help("Filter to a specific profile")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("raw")
                .long("raw")
                .help("Show values without region/profile labels"),
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("Include instances without IPs"),
        )
        .get_matches();

    // Assign argument options
    let raw = matches.is_present("raw");
    let all = matches.is_present("all");

    // Retrieve credentials from file
    let creds_file_lines = read_credentials_file();
    let creds = match creds_file_lines {
        Ok(lines) => aws_creds_list(lines),
        Err(_) => {
            eprintln!("Error reading AWS credentials file. Please check that it's correct.");
            std::process::exit(1);
        }
    };

    // Request all the clients in each region for an AWS account
    let mut clients_futs = FuturesUnordered::new();
    for cred in creds {
        // Or filter based on region argument
        if let Some(profile) = matches.value_of("profile") {
            if &cred.name != profile {
                continue;
            }
        }
        clients_futs.push(regional_clients(cred));
    }

    // Process the client results and request instance IPs
    let mut futs = FuturesOrdered::new();
    while let Some(regional_clients) = clients_futs.next().await {
        for regional_client in regional_clients {
            if let Some(region_name) = matches.value_of("region") {
                if regional_client.region.name() != region_name {
                    continue;
                }
            }
            futs.push(regional_instances(regional_client, all));
        }
    }

    // Process and print results
    let mut current_profile = String::new();
    while let Some(instances_result) = futs.next().await {
        match instances_result {
            Ok(instances_collection) => {
                // Only start printing if at least 1 instance
                if instances_collection.metadatas.len() > 0 {
                    // Print the profile name if first time seeing it (and not raw mode)
                    if instances_collection.profile != current_profile {
                        if !raw {
                            // First line is not a new line
                            if current_profile != "" {
                                println!("");
                            }
                            println!("[{}]", instances_collection.profile.green().bold());
                        }
                        current_profile = instances_collection.profile;
                    }
                    let region = instances_collection.region.name();
                    // Print regional breakers
                    if !raw {
                        let horiz: String = (0..&region.len() + 2).map(|_| '-').collect();
                        println!("{}", &horiz);
                        println!("|{}|", region.yellow());
                        println!("{}", &horiz);
                    }
                    for instance in instances_collection.metadatas {
                        println!("{} - {}", instance.name, instance.ip,);
                    }
                }
            }
            Err(error) => {
                eprintln!("Error: {}", error.red());
            }
        }
    }
}

async fn regional_clients(credential: NamedStaticProvider) -> Vec<RegionalClient> {
    let mut ec2_regions: Vec<rusoto_ec2::Region> = Vec::new();
    {
        let client = Ec2Client::new_with(
            HttpClient::new().unwrap(),
            credential.provider.clone(),
            Region::UsEast1,
        );

        let regions_request = DescribeRegionsRequest {
            all_regions: None,
            dry_run: None,
            filters: None,
            region_names: None,
        };

        let result = client.describe_regions(regions_request).await;
        ec2_regions = match result {
            Ok(regions_result) => match regions_result.regions {
                Some(regions) => regions,
                None => {
                    eprintln!("No regions found");
                    ec2_regions
                }
            },
            Err(_) => {
                eprintln!("Error getting regions");
                ec2_regions
            }
        };
    }

    let clients = ec2_regions
        .into_iter()
        .map(|ec2_region| {
            let region_name = match ec2_region.region_name {
                Some(name) => name,
                None => {
                    eprintln!("Region name not found");
                    std::process::exit(1);
                }
            };
            let region = match Region::from_str(&region_name) {
                Ok(region) => region,
                Err(_) => {
                    eprintln!("Region cannot be parsed");
                    std::process::exit(1);
                }
            };
            let client = Ec2Client::new_with(
                HttpClient::new().unwrap(),
                credential.provider.clone(),
                region.clone(),
            );
            RegionalClient {
                region,
                client,
                profile: credential.name.clone(),
            }
        })
        .collect();

    clients
}

async fn regional_instances(
    regional_client: RegionalClient,
    all: bool,
) -> Result<InstanceMetadataCollection, String> {
    let describe_instances_input: DescribeInstancesRequest = DescribeInstancesRequest {
        dry_run: None,
        filters: None,
        instance_ids: None,
        max_results: None,
        next_token: None,
    };

    let result = match regional_client
        .client
        .describe_instances(describe_instances_input)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            return Err(format!(
                "Region failure in {}: {}",
                &regional_client.region.name(),
                error
            ))
        }
    };

    let reservations = match result.reservations {
        Some(reservations) => reservations,
        None => return Err(String::from("No reservations")),
    };

    let total_instances: Vec<Instance> = reservations
        .into_iter()
        .filter_map(|reservation| reservation.instances)
        .flatten()
        .collect();

    let mut instance_metadatas = InstanceMetadataCollection {
        metadatas: Vec::new(),
        profile: regional_client.profile,
        region: regional_client.region,
    };

    for instance in total_instances {
        let ip_address = match &instance.public_ip_address {
            Some(ip_address) => ip_address,
            None => match all {
                true => "N/A",
                false => continue,
            },
        };

        let name = instance_name(&instance);

        instance_metadatas.metadatas.push(InstanceMetadata {
            name,
            ip: String::from(ip_address),
        });
    }

    Ok(instance_metadatas)
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
        .map(|line| line.unwrap_or_else(|_| String::from("")))
        .collect();
    Ok(lines)
}

fn aws_creds_list(lines: Vec<String>) -> Vec<NamedStaticProvider> {
    let mut key = None;
    let mut secret = None;
    let mut creds = Vec::new();
    let mut profile = None;
    for line in lines {
        let mut words = line.split('=');
        if let Some(word) = words.next() {
            match word.trim() {
                "aws_access_key_id" => {
                    key = Some(words.next().unwrap().trim().to_string());
                }
                "aws_secret_access_key" => {
                    secret = Some(words.next().unwrap().trim().to_string());
                }
                other_word => {
                    if other_word.starts_with("[") {
                        profile = Some(other_word.replace("[", "").replace("]", ""));
                    }
                }
            }
        }

        match (key.take(), secret.take(), profile.take()) {
            (Some(key_value), Some(secret_value), Some(profile_value)) => {
                let provider = StaticProvider::new(key_value, secret_value, None, None);
                creds.push(NamedStaticProvider {
                    name: profile_value,
                    provider,
                });
            }
            (taken_key, taken_secret, taken_profile) => {
                key = taken_key;
                secret = taken_secret;
                profile = taken_profile;
            }
        }
    }
    creds
}

fn instance_name(instance: &Instance) -> String {
    let fallback = "N/A";
    match &instance.tags {
        Some(tags) => {
            for tag in tags {
                match &tag.key {
                    Some(key) => {
                        if key.as_str() == "Name" {
                            match &tag.value {
                                Some(value) => {
                                    return value.to_owned();
                                }
                                None => (),
                            }
                        }
                    }
                    None => (),
                }
            }
            String::from(fallback)
        }
        None => String::from(fallback),
    }
}
