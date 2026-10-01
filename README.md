## Access code automation

I want to automate the whole access process. I don't want to open my phone, authenticate, write in the name of the visitor and address
every single time.

So the goal is to write a rust server, upload the binary to a vps and then use that with Apple shortcut.

The final solution is:
-> Get Access code(name) => Returns Access code

## Steps that would involve

1. Load envs from environment
   -> throw when not available
2. Make request to server to check availability
   -> return error when not available
3. If server is available, return { name, code }
   -> return error if returned name doesn't match
   -> return error if site is down
