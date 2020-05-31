# aws_ips

Get all AWS EC2 IPs in all regions using all credentials in credentials file

This program will do the following:

- Extract all profiles from your home credentials file
- Query AWS in all regions for each profile simultaneously
- Collect EC2 instances and check for a `Name` tag and public IP
- Return results asynchronously

Sometimes the AWS API has a fit or stalls and you may need to run the program a second time.
