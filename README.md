# justrun
Just run it! A simple scheduler system.

## Usage
Install as root:
```
sudo cargo install justrun --root /usr/local/
```
Or install as user and copy:
```
cargo install justrun
which justrun
sudo cp JUSTRUN_PATH /usr/local/bin
which justrund
sudo cp JUSTRUND_PATH /usr/local/bin
```

### Daemon
Create a service for the daemon. Example for SystemD:
```
[Unit]
Description=Justrun Daemon
After=default.target
Wants=default.target

[Service]
ExecStart=REPLACE WITH which justrund
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
User=root

[Install]
WantedBy=default.target
```

### Add services
Justrun requires a yaml config file to launch daemons.

`justrun.yaml`
```
name: example
start: 'echo "Hello, world!"'
```

Register the directory with the config:
```
sudo justrun reg
```

Restart the `justrund` daemon:
```
sudo justrun restartd
```

Check for errors:
```
sudo justrun status example
```
