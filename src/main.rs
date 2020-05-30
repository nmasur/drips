use rusoto_core::Region;
use rusoto_ec2::{DescribeInstancesRequest, Ec2, Ec2Client, Instance};

#[tokio::main]
async fn main() {
    // let regions: [Region; 4] = [
    //     Region::UsEast1,
    //     Region::UsEast2,
    //     Region::UsWest1,
    //     Region::UsWest2,
    // ];
    let client = Ec2Client::new(Region::UsWest2);
    let describe_instances_input: DescribeInstancesRequest = DescribeInstancesRequest {
        dry_run: None,
        filters: None,
        instance_ids: None,
        max_results: None,
        next_token: None,
    };

    let output = match client.describe_instances(describe_instances_input).await {
        Ok(output) => {
            println!("Got something");
            output
        }
        Err(error) => {
            eprintln!("Got error: {}", error);
            std::process::exit(1)
        }
    };

    let reservations = match output.reservations {
        Some(reservation) => {
            println!("Got reservation");
            reservation
        }
        None => {
            println!("No reservation");
            std::process::exit(1)
        }
    };

    // let instances = reservations
    //     .map(|reservation| match reservation.instances {
    //         Some(instances) => instances,
    //         None => Vec::new(),
    //     })
    //     .collect::<Vec<Instance>>();

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

    println!("Total instances: {}", total_instances.len());

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

        println!("{}: {}", name, ip_address);
    }
}
