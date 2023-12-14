# busypot
This tool offers a mix of useful commands for interacting with Nodle parachain or any other compatible substrate based parachain. 
The commands specially include `propose-xcm` which allows a member of technical committee to propose a native transaction 
on the relay chain on behalf of Nodle. 

## Usage
Here are a few useful commands:
```
# Create 3 pots with pot ids as 0, 1, 2
busypot -u "ws://localhost:9280" create-pots 3

# Register 3 users for pot 0 all derived from //Alice
busypot -u "ws://localhost:9280" regiseter-users --pot-id 0 --users 3

# Propose unlocking parachain 2000 but don't send the transaction, just print it
busypot -u "ws://localhost:9280" propose-xcm --transact "4604ea070000" --dry-run
```
In the above commands "ws://localhost:9280" is the rpc endpoint of a parachain's node.

