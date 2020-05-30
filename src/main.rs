use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use rusoto_core::Region;
use rusoto_ec2::{DescribeInstancesRequest, DescribeInstancesResult, Ec2, Ec2Client, Instance};

struct InstanceMetadata {
    name: String,
    ip: String,
    region: Region,
}

#[tokio::main]
async fn main() {
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
    for region in regions {
        // println!("Region: {}", &region.name());
        futs.push(regional_instances(region))
    }

    while let Some(instances) = futs.next().await {
        for instance in instances {
            println!(
                "Instance: {} - {} ({})",
                instance.name,
                instance.ip,
                instance.region.name()
            );
        }
    }

    // let future_instances_by_region: Vec<_> = regions
    //     .into_iter()
    //     .map(|region| regional_instances(region))
    //     .collect();

    // for future_instances in future_instances_by_region {
    //     let instances = future_instances.await;
    //     for instance in instances {
    //         println!("Instance: {} - {}", instance.name, instance.ip);
    //     }
    // }

    // for future_instances in future_instances_by_region {
    //     let instances_description = future_instances.await;
    //     let instances = regional_instances(instances_description);
    //     for instance in instances {
    //         println!("Instance: {} - {}", instance.name, instance.ip);
    //     }
    // }

    // while let Some(instances) = future_instances_by_region.into_iter().next().await {
    //     println!("{}", "Thing");
    //     instances.map(|instance| {
    //         println!("Instance: {} - {}", instance.name, instance.ip);
    //     })
    // }
    // let instances_us_west_2 = regional_instances(Region::UsWest2);
    // println!("{}", Region::UsWest2.name());
    // for instance in instances_us_west_2.await {
    //     println!("Instance: {} - {}", instance.name, instance.ip);
    // }
}

// async fn describe_instances(region: Region) -> DescribeInstancesResult {
//     let client = Ec2Client::new(region);
//     let describe_instances_input: DescribeInstancesRequest = DescribeInstancesRequest {
//         dry_run: None,
//         filters: None,
//         instance_ids: None,
//         max_results: None,
//         next_token: None,
//     };

//     client.describe_instances(describe_instances_input)
// }

async fn regional_instances(region: Region) -> Vec<InstanceMetadata> {
    let client = Ec2Client::new(region.clone());
    let describe_instances_input: DescribeInstancesRequest = DescribeInstancesRequest {
        dry_run: None,
        filters: None,
        instance_ids: None,
        max_results: None,
        next_token: None,
    };

    let output = match client.describe_instances(describe_instances_input).await {
        Ok(output) => output,
        Err(_) => {
            eprintln!("{} - ERROR - Failed", &region.name());
            // std::process::exit(1)
            DescribeInstancesResult {
                next_token: None,
                reservations: None,
            }
        }
    };

    let reservations = match output.reservations {
        Some(reservations) => reservations,
        None => {
            println!("No reservation");
            // std::process::exit(1)
            Vec::new()
        }
    };

    let mut total_instances: Vec<Instance> = Vec::new();
    for reservation in reservations {
        match reservation.instances {
            Some(instances) => {
                for instance in instances {
                    total_instances.push(instance);
                }
            }
            None => (),
        }
    }

    println!(
        "{}: total instances: {}",
        &region.name(),
        total_instances.len()
    );

    let mut instance_metadatas = Vec::new();

    for instance in total_instances {
        let ip_address = match &instance.public_ip_address {
            Some(ip_address) => ip_address,
            None => "N/A",
        };

        let name = match &instance.tags {
            Some(tags) => {
                let mut name_tag = String::from("N/A");
                for tag in tags {
                    match &tag.key {
                        Some(key) => {
                            if key.as_str() == "Name" {
                                match &tag.value {
                                    Some(value) => {
                                        name_tag = value.clone();
                                        break;
                                    }
                                    None => (),
                                }
                            }
                        }
                        None => (),
                    }
                }
                name_tag
            }
            None => String::from("N/A"),
        };

        // println!("{}: {} ({})", name, ip_address, &region.name());
        instance_metadatas.push(InstanceMetadata {
            name,
            ip: String::from(ip_address),
            region: region.clone(),
        });
    }

    instance_metadatas
}
