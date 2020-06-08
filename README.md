# DRIPS: Dammit, Retrieve IPS (from AWS)

Lists all AWS EC2 public IPs in all regions using all credentials in credentials file

## What?

_drips_ does the following:

- Extract all profiles from your home credentials file
  - `~/.aws/credentials` on macOS/Linux
  - `%UserProfile%\.aws\credentials` on Windows
- Collect EC2 instances in all accounts and regions with a `Name` tag and public IP
  - Instances without a name tag are titled "N/A"
  - Data is requested and returned asynchronously

## Why?

I just need the IPs and I have too many accounts / too lazy to login.

## With?

_drips_ includes the following optional parameters:

- `--profile <PROFILE NAME>`: Filter to a specific profile name
- `--region <REGION NAME>`: Filter to a specific region name
- `--all`: Include EC2s that don't have a public IP
- `--raw`: Only show list values, don't label profiles/regions

## Where?

See [releases](https://github.com/nmasur/drips/releases) page for binaries.

On MacOS, you can also install from Homebrew:

```
brew install nmasur/homebrew/drips
```

Alternatively, build from source using [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):

```
git clone git://github.com/nmasur/drips
cd drips
cargo build --release
```
